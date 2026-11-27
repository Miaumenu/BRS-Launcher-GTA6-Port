//! What the host GPU reports, for choosing a preset.
//!
//! This is a throwaway query: it creates a device, reads the capabilities a
//! preset can match on, and drops everything. It deliberately does not go
//! through `gambiarra-gpu`'s `Host`, whose device is the one the title runs
//! on and whose creation has side effects (caches, debug layers, the shader
//! translator's host settings).
//!
//! Every field is read on its own and left unset when its query fails. An
//! unset capability is not a guess: [`gambiarra_core::preset::Match`] refuses
//! any preset that constrains a capability we could not read.

use gambiarra_core::preset::{Capabilities, ShaderModel, Vendor};

/// Reads the host's capabilities, as far as the driver answers.
///
/// Never fails: what could not be read stays unset, and the caller falls back
/// to the baseline preset.
pub fn probe() -> Capabilities {
    #[cfg(windows)]
    {
        windows_probe()
    }
    #[cfg(not(windows))]
    {
        // No D3D12 here; the baseline preset is the honest answer.
        Capabilities::default()
    }
}

#[cfg(windows)]
fn windows_probe() -> Capabilities {
    use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
    use windows::Win32::Graphics::Direct3D12::{
        D3D_SHADER_MODEL, D3D12_FEATURE_D3D12_OPTIONS1, D3D12_FEATURE_D3D12_OPTIONS7,
        D3D12_FEATURE_DATA_D3D12_OPTIONS1, D3D12_FEATURE_DATA_D3D12_OPTIONS7,
        D3D12_FEATURE_DATA_SHADER_MODEL, D3D12_FEATURE_SHADER_MODEL,
        D3D12_MESH_SHADER_TIER_NOT_SUPPORTED, D3D12CreateDevice, ID3D12Device,
    };
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, IDXGIAdapter1, IDXGIFactory4,
    };

    let mut caps = Capabilities::default();

    // SAFETY: each call below is a plain query. `CheckFeatureSupport` is given
    // a pointer to a local structure together with that structure's own size,
    // and the device and adapter are released when this function returns.
    unsafe {
        let mut device: Option<ID3D12Device> = None;
        // The default adapter, which is the one a run would get.
        if D3D12CreateDevice(None, D3D_FEATURE_LEVEL_11_0, &mut device).is_err() {
            return caps;
        }
        let Some(device) = device else {
            return caps;
        };

        // Vendor and memory come from the adapter behind the device, found by
        // the device's own LUID so that it is the same adapter, not merely
        // the first one DXGI lists.
        if let Ok(factory) = CreateDXGIFactory2::<IDXGIFactory4>(DXGI_CREATE_FACTORY_FLAGS(0))
            && let Ok(adapter) = factory.EnumAdapterByLuid::<IDXGIAdapter1>(device.GetAdapterLuid())
            && let Ok(desc) = adapter.GetDesc1()
        {
            caps.vendor = Vendor::from_pci_id(desc.VendorId);
            caps.vram_mb = Some((desc.DedicatedVideoMemory as u64) / (1024 * 1024));
        }

        let mut o1 = D3D12_FEATURE_DATA_D3D12_OPTIONS1::default();
        if device
            .CheckFeatureSupport(
                D3D12_FEATURE_D3D12_OPTIONS1,
                (&raw mut o1).cast(),
                size_of_val(&o1) as u32,
            )
            .is_ok()
        {
            caps.wave_lanes_min = Some(o1.WaveLaneCountMin);
        }

        let mut o7 = D3D12_FEATURE_DATA_D3D12_OPTIONS7::default();
        if device
            .CheckFeatureSupport(
                D3D12_FEATURE_D3D12_OPTIONS7,
                (&raw mut o7).cast(),
                size_of_val(&o7) as u32,
            )
            .is_ok()
        {
            caps.mesh_shader = Some(o7.MeshShaderTier != D3D12_MESH_SHADER_TIER_NOT_SUPPORTED);
        }

        // Asking for a model the installed runtime does not know is an error
        // rather than a clamp, so walk down from the newest until one answers.
        // The values are `0x<major><minor>`, so 6.9 is 0x69.
        for minor in (0..=9u8).rev() {
            let mut model = D3D12_FEATURE_DATA_SHADER_MODEL {
                HighestShaderModel: D3D_SHADER_MODEL(0x60 | i32::from(minor)),
            };
            if device
                .CheckFeatureSupport(
                    D3D12_FEATURE_SHADER_MODEL,
                    (&raw mut model).cast(),
                    size_of_val(&model) as u32,
                )
                .is_ok()
            {
                // On success the runtime has written back the highest it
                // actually supports, which may be below what we asked.
                let value = model.HighestShaderModel.0;
                caps.shader_model = Some(ShaderModel::new(
                    ((value >> 4) & 0xF) as u8,
                    (value & 0xF) as u8,
                ));
                break;
            }
        }
    }

    caps
}

/// The capabilities as a line for a log or a report.
pub fn describe(caps: &Capabilities) -> String {
    let unknown = "?".to_string();
    format!(
        "vendor={} mesh_shader={} shader_model={} wave_lanes_min={} vram_mb={}",
        caps.vendor
            .map(|v| v.to_string())
            .unwrap_or(unknown.clone()),
        caps.mesh_shader
            .map(|v| v.to_string())
            .unwrap_or(unknown.clone()),
        caps.shader_model
            .map(|v| v.to_string())
            .unwrap_or(unknown.clone()),
        caps.wave_lanes_min
            .map(|v| v.to_string())
            .unwrap_or(unknown.clone()),
        caps.vram_mb.map(|v| v.to_string()).unwrap_or(unknown),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_capabilities_describe_as_question_marks() {
        let line = describe(&Capabilities::default());
        assert_eq!(
            line,
            "vendor=? mesh_shader=? shader_model=? wave_lanes_min=? vram_mb=?"
        );
    }

    #[test]
    fn a_full_set_describes_every_field() {
        let caps = Capabilities {
            vendor: Some(Vendor::Amd),
            mesh_shader: Some(false),
            shader_model: Some(ShaderModel::new(6, 5)),
            wave_lanes_min: Some(64),
            vram_mb: Some(2027),
        };
        assert_eq!(
            describe(&caps),
            "vendor=amd mesh_shader=false shader_model=6.5 wave_lanes_min=64 vram_mb=2027"
        );
    }

    #[test]
    fn probing_this_machine_never_panics() {
        // Whatever the host is (including no D3D12 at all), this must return.
        let _ = probe();
    }
}
