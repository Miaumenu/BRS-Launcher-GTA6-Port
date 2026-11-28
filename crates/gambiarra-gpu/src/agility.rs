//! Optional D3D12 Agility SDK loading via `ID3D12SDKConfiguration1`.
//!
//! Windows 10's inbox `System32\D3D12Core.dll` stops at Shader Model 6.5, even when
//! the GPU and driver support SM 6.6+. Windows 10 22H2 (build 19045.3393+, KB5029331)
//! and Windows 11 expose `ID3D12SDKConfiguration1::CreateDeviceFactory`, which loads an
//! app-supplied Agility SDK `D3D12Core.dll` at run time without Developer Mode and
//! without static `D3D12SDKVersion` / `D3D12SDKPath` PE exports.
//!
//! **Why this matters for GambiarraLauncher:** translated shaders use SM 6.6 bindless
//! heap access, which requires flags
//! `D3D12_ROOT_SIGNATURE_FLAG_CBV_SRV_UAV_HEAP_DIRECTLY_INDEXED` and
//! `D3D12_ROOT_SIGNATURE_FLAG_SAMPLER_HEAP_DIRECTLY_INDEXED` in root signatures.
//! On the inbox DLL these flags are unrecognised by `D3D12SerializeVersionedRootSignature`
//! and `CreateRootSignature` returns `E_INVALIDARG`, failing every pipeline — even on WARP.
//! Calling `ID3D12DeviceFactory::ApplyToGlobalState` upgrades the process-wide D3D12
//! state so that free functions (including `D3D12SerializeVersionedRootSignature`)
//! dispatch to the Agility SDK's copy, which knows about these flags.
//!
//! Search order for `D3D12Core.dll` (first valid one wins):
//! 1. `%GAMBIARRA_D3D12_DIR%` (when set and non-empty),
//! 2. `<exe_dir>\D3D12`,
//! 3. `<exe_dir>`,
//! 4. `%LOCALAPPDATA%\GambiarraLauncher\D3D12`.
//!
//! If no Agility SDK is found, or the OS lacks `ID3D12SDKConfiguration1` (Windows 10
//! older than 22H2 KB5029331), device creation falls back transparently to inbox
//! `D3D12CreateDevice`. Root signatures will then fail on SM 6.6 flags.

use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use windows::core::{Interface, IUnknown, Param, GUID, PCSTR};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL;
use windows::Win32::Graphics::Direct3D12::*;

/// Environment variable overriding the Agility SDK folder.
pub const ENV: &str = "GAMBIARRA_D3D12_DIR";

/// Result of attempting to load the Agility SDK once for this process.
#[derive(Debug)]
pub enum State {
    /// Loaded through `ID3D12SDKConfiguration1::CreateDeviceFactory`.
    Loaded {
        factory: ID3D12DeviceFactory,
        version: u32,
        dir: PathBuf,
    },
    /// No Agility SDK DLL was found in any search path; inbox D3D12 will be used.
    NotFound,
    /// An Agility SDK was found or requested, but could not be activated; inbox D3D12 will be used.
    Unavailable(String),
}

static STATE: OnceLock<State> = OnceLock::new();

/// Process-wide Agility SDK state (initialized on first D3D12 use, before any device is created).
pub fn state() -> &'static State {
    STATE.get_or_init(init)
}

/// Active Agility SDK `(version, dir)` if loaded, otherwise `None`.
pub fn loaded() -> Option<(u32, &'static Path)> {
    match state() {
        State::Loaded { version, dir, .. } => Some((*version, dir.as_path())),
        _ => None,
    }
}

/// Create a D3D12 device through the Agility SDK factory when loaded, otherwise via
/// inbox `D3D12CreateDevice`. Pass `None::<IDXGIAdapter1>` to let D3D12 pick the adapter.
pub fn create_device<P>(adapter: P, feature_level: D3D_FEATURE_LEVEL) -> windows::core::Result<ID3D12Device>
where
    P: Param<IUnknown> + Copy,
{
    let mut device: Option<ID3D12Device> = None;
    match state() {
        State::Loaded { factory, .. } => {
            // SAFETY: `factory` is a valid `ID3D12DeviceFactory` kept alive for `'static`,
            // and `device` is a valid out-pointer for `ID3D12Device`.
            unsafe { factory.CreateDevice(adapter, feature_level, &mut device)? };
        }
        _ => {
            // SAFETY: standard `D3D12CreateDevice` call with valid out-pointer.
            unsafe { D3D12CreateDevice(adapter, feature_level, &mut device)? };
        }
    }
    device.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))
}

/// Create a D3D12 device on the default adapter (let D3D12 choose).
///
/// After `state()` is called once, `ApplyToGlobalState()` has upgraded the process-wide
/// D3D12 DLL, so `D3D12CreateDevice(None, ...)` already dispatches through the Agility
/// SDK.  We therefore always use `D3D12CreateDevice` here; the factory path is used only
/// when a specific adapter handle is provided (via `create_device`).
pub fn create_default_device(feature_level: D3D_FEATURE_LEVEL) -> windows::core::Result<ID3D12Device> {
    // Ensure the Agility SDK is loaded and ApplyToGlobalState has been called.
    let _ = state();
    let mut device: Option<ID3D12Device> = None;
    // SAFETY: standard `D3D12CreateDevice(None, ...)` call.  After `ApplyToGlobalState`
    // this dispatches to the Agility SDK's D3D12Core.dll instead of the inbox copy.
    unsafe { D3D12CreateDevice(None, feature_level, &mut device)? };
    device.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))
}

/// Retrieve a D3D12 configuration/debug interface from the Agility SDK factory when
/// loaded, otherwise from inbox `D3D12GetDebugInterface`.
pub fn configuration_interface<T: Interface>(clsid: &GUID) -> windows::core::Result<T> {
    match state() {
        State::Loaded { factory, .. } => {
            // SAFETY: `factory` is a valid `ID3D12DeviceFactory` kept alive for `'static`.
            unsafe { factory.GetConfigurationInterface::<T>(clsid) }
        }
        _ => {
            let mut out: Option<T> = None;
            // SAFETY: `D3D12GetDebugInterface` writes a COM pointer on success.
            unsafe { D3D12GetDebugInterface(&mut out)? };
            out.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_NOINTERFACE))
        }
    }
}

// ─── internals ───────────────────────────────────────────────────────────────

fn init() -> State {
    let env_dir = env_path(ENV);
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let local_app_data = env_path("LOCALAPPDATA");

    let (dir, version) = match resolve_dir(env_dir, exe_dir, local_app_data) {
        Resolution::Found { dir, version } => (dir, version),
        Resolution::NotFound => {
            tracing::debug!("Agility SDK not found; using inbox D3D12 (SM 6.6 root signatures may fail)");
            return State::NotFound;
        }
        Resolution::Unavailable(reason) => {
            tracing::warn!(reason, "Agility SDK unavailable; using inbox D3D12");
            return State::Unavailable(reason);
        }
    };

    // `CreateDeviceFactory` takes an ANSI path with a trailing backslash.
    let abs_dir = std::path::absolute(&dir).unwrap_or_else(|_| dir.clone());
    let path_cstr = match sdk_path_cstring(&abs_dir) {
        Ok(p) => p,
        Err(err) => {
            tracing::warn!(error = err, "Agility SDK path could not be encoded");
            return State::Unavailable(err);
        }
    };

    match create_factory(version, &abs_dir, &path_cstr) {
        Ok(factory) => {
            tracing::info!(version, dir = %abs_dir.display(), "D3D12 Agility SDK loaded");
            State::Loaded { factory, version, dir: abs_dir }
        }
        Err(reason) => {
            tracing::warn!(reason, "Agility SDK CreateDeviceFactory failed; using inbox D3D12");
            State::Unavailable(reason)
        }
    }
}

fn env_path(var: &str) -> Option<PathBuf> {
    let v = std::env::var_os(var)?;
    if v.is_empty() {
        return None;
    }
    if let Some(s) = v.to_str() {
        let t = s.trim();
        if t.is_empty() {
            return None;
        }
        return Some(PathBuf::from(t));
    }
    Some(PathBuf::from(v))
}

#[derive(Debug)]
enum Resolution {
    Found { dir: PathBuf, version: u32 },
    NotFound,
    Unavailable(String),
}

fn resolve_dir(
    env_override: Option<PathBuf>,
    exe_dir: Option<PathBuf>,
    local_app_data: Option<PathBuf>,
) -> Resolution {
    let explicit = env_override.is_some();
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(d) = env_override {
        dirs.push(d);
        // When GAMBIARRA_D3D12_DIR is set, do not fall through to unrelated folders.
        return probe_dirs(&dirs, true);
    }
    if let Some(d) = &exe_dir {
        dirs.push(d.join("D3D12"));
        dirs.push(d.clone());
    }
    if let Some(d) = local_app_data {
        dirs.push(d.join("GambiarraLauncher").join("D3D12"));
    }
    probe_dirs(&dirs, explicit)
}

fn probe_dirs(dirs: &[PathBuf], stop_on_error: bool) -> Resolution {
    let mut last_err: Option<String> = None;
    for dir in dirs {
        let dll = dir.join("D3D12Core.dll");
        if !dll.is_file() {
            continue;
        }
        match read_sdk_version(dir) {
            Ok(version) => return Resolution::Found { dir: dir.clone(), version },
            Err(err) => {
                last_err = Some(err);
                if stop_on_error {
                    break;
                }
            }
        }
    }
    match last_err {
        Some(err) => Resolution::Unavailable(err),
        None => Resolution::NotFound,
    }
}

/// Read the `D3D12SDKVersion` data export from `<dir>\D3D12Core.dll` without loading it.
fn read_sdk_version(dir: &Path) -> Result<u32, String> {
    let dll = dir.join("D3D12Core.dll");
    let bytes = std::fs::read(&dll).map_err(|e| format!("{}: {e}", dll.display()))?;
    read_sdk_version_from_bytes(&bytes).map_err(|e| format!("{}: {e}", dll.display()))
}

/// Parse a minimal PE64 export table to find the `D3D12SDKVersion` data export (a `u32`).
fn read_sdk_version_from_bytes(bytes: &[u8]) -> Result<u32, String> {
    // DOS header: e_lfanew at +0x3c
    let pe_off = read_u32(bytes, 0x3c)? as usize;
    if bytes.get(pe_off..pe_off + 4) != Some(b"PE\0\0") {
        return Err("not a PE file".into());
    }
    // Machine: 0x8664 = AMD64
    if read_u16(bytes, pe_off + 4)? != 0x8664 {
        return Err("not an x64 PE".into());
    }
    let opt_hdr_size = read_u16(bytes, pe_off + 20)? as usize;
    // DataDirectory[0] = Export at optional header offset +112 (0x70)
    let opt = pe_off + 24;
    if opt_hdr_size < 0x78 {
        return Err("optional header too small to have export directory".into());
    }
    let exp_rva = read_u32(bytes, opt + 0x70)? as usize;
    if exp_rva == 0 {
        return Err("no export directory".into());
    }
    let num_sections = read_u16(bytes, pe_off + 6)? as usize;
    let sect_off = opt + opt_hdr_size;

    let rva_to_off = |rva: usize| -> Option<usize> {
        for i in 0..num_sections {
            let s = sect_off + i * 40;
            let va = u32::from_le_bytes(bytes.get(s + 12..s + 16)?.try_into().ok()?) as usize;
            let vs = u32::from_le_bytes(bytes.get(s + 16..s + 20)?.try_into().ok()?) as usize;
            let raw = u32::from_le_bytes(bytes.get(s + 20..s + 24)?.try_into().ok()?) as usize;
            if rva >= va && rva < va + vs.max(1) {
                return Some(raw + rva - va);
            }
        }
        None
    };

    let exp_off = rva_to_off(exp_rva).ok_or("export directory RVA not in any section")?;
    // IMAGE_EXPORT_DIRECTORY:
    //  +20: NumberOfFunctions, +24: NumberOfNames
    //  +28: AddressOfFunctions (RVA), +32: AddressOfNames (RVA), +36: AddressOfNameOrdinals (RVA)
    let n_names = read_u32(bytes, exp_off + 24)? as usize;
    let fns_off = rva_to_off(read_u32(bytes, exp_off + 28)? as usize)
        .ok_or("functions RVA not in any section")?;
    let names_off = rva_to_off(read_u32(bytes, exp_off + 32)? as usize)
        .ok_or("names RVA not in any section")?;
    let ords_off = rva_to_off(read_u32(bytes, exp_off + 36)? as usize)
        .ok_or("ordinals RVA not in any section")?;

    for i in 0..n_names {
        let name_rva = read_u32(bytes, names_off + i * 4)? as usize;
        let name_off = rva_to_off(name_rva).ok_or("name RVA not in any section")?;
        let name = bytes[name_off..].split(|&b| b == 0).next().unwrap_or(&[]);
        if name != b"D3D12SDKVersion" {
            continue;
        }
        let ord = read_u16(bytes, ords_off + i * 2)? as usize;
        let fn_rva = read_u32(bytes, fns_off + ord * 4)? as usize;
        let fn_off = rva_to_off(fn_rva).ok_or("D3D12SDKVersion data RVA not in any section")?;
        return read_u32(bytes, fn_off);
    }
    Err("D3D12SDKVersion export not found".into())
}

#[inline]
fn read_u16(b: &[u8], off: usize) -> Result<u16, String> {
    b.get(off..off + 2)
        .and_then(|s| s.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or_else(|| format!("PE read out of bounds at +{off:#x}"))
}

#[inline]
fn read_u32(b: &[u8], off: usize) -> Result<u32, String> {
    b.get(off..off + 4)
        .and_then(|s| s.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| format!("PE read out of bounds at +{off:#x}"))
}

/// Format a directory path as a NUL-terminated ANSI string with a trailing backslash,
/// as required by `ID3D12SDKConfiguration1::CreateDeviceFactory`.
/// Paths in GAMBIARRA_D3D12_DIR, the exe directory and LOCALAPPDATA are typically ASCII.
fn sdk_path_cstring(dir: &Path) -> Result<CString, String> {
    let s = dir
        .to_str()
        .ok_or("Agility SDK path is not valid UTF-8")?
        .replace('/', "\\");
    let s = if s.ends_with('\\') { s } else { s + "\\" };
    CString::new(s.into_bytes()).map_err(|_| "Agility SDK path contains a NUL byte".into())
}

type D3D12GetInterfaceFn = unsafe extern "system" fn(
    *const GUID,
    *const GUID,
    *mut *mut std::ffi::c_void,
) -> windows::core::HRESULT;

fn resolve_d3d12_get_interface() -> Result<D3D12GetInterfaceFn, String> {
    use windows_sys::Win32::System::LibraryLoader::{
        GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
    };
    // Encode "d3d12.dll" as UTF-16 NUL-terminated.
    let name: Vec<u16> = "d3d12.dll\0".encode_utf16().collect();
    // SAFETY: `name` is a valid NUL-terminated UTF-16 string; flag is a constant.
    let h = unsafe { LoadLibraryExW(name.as_ptr(), std::ptr::null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32) };
    if h.is_null() {
        return Err("failed to load d3d12.dll from System32".into());
    }
    // SAFETY: `h` is a valid module handle; the name is a NUL-terminated byte literal.
    let sym = unsafe { GetProcAddress(h, b"D3D12GetInterface\0".as_ptr()) };
    sym.ok_or_else(|| {
        "D3D12GetInterface not available (update Windows 10 to 22H2 KB5029331+)".into()
    })
    // SAFETY: the function pointer type matches the Win32 export signature.
    .map(|f| unsafe { std::mem::transmute::<_, D3D12GetInterfaceFn>(f) })
}

fn create_factory(
    version: u32,
    dir: &Path,
    path_cstr: &CString,
) -> Result<ID3D12DeviceFactory, String> {
    // `D3D12GetInterface` is resolved dynamically so older Windows 10 builds do not
    // fail at IAT load time. It writes a COM interface pointer into `cfg_raw`.
    // `ID3D12SDKConfiguration1::CreateDeviceFactory` receives a valid NUL-terminated ANSI
    // path with a trailing backslash and returns an owned `ID3D12DeviceFactory`.
    let get_iface = resolve_d3d12_get_interface()?;
    unsafe {
        let mut cfg_raw: *mut std::ffi::c_void = std::ptr::null_mut();
        get_iface(
            &CLSID_D3D12SDKConfiguration,
            &ID3D12SDKConfiguration1::IID,
            &mut cfg_raw,
        )
        .ok()
        .map_err(|e| {
            format!(
                "ID3D12SDKConfiguration1 unavailable (update Windows 10 to 22H2 KB5029331+): {e}"
            )
        })?;
        if cfg_raw.is_null() {
            return Err("D3D12GetInterface returned a null ID3D12SDKConfiguration1".into());
        }
        let cfg = ID3D12SDKConfiguration1::from_raw(cfg_raw);
        let factory: ID3D12DeviceFactory = cfg
            .CreateDeviceFactory(version, PCSTR(path_cstr.as_ptr().cast()))
            .map_err(|e| format!("CreateDeviceFactory(v{version}, {}): {e}", dir.display()))?;
        factory
            .SetFlags(D3D12_DEVICE_FACTORY_FLAG_ALLOW_RETURNING_EXISTING_DEVICE)
            .map_err(|e| format!("ID3D12DeviceFactory::SetFlags: {e}"))?;
        // Critical: upgrades process-wide D3D12 global state so that free functions that
        // are NOT exposed on ID3D12DeviceFactory — notably D3D12SerializeVersionedRootSignature,
        // which must recognise SM 6.6 heap-indexing flags (0x400|0x800) — dispatch to the
        // Agility SDK's D3D12Core.dll instead of the Windows 10 inbox copy.
        factory
            .ApplyToGlobalState()
            .map_err(|e| format!("ID3D12DeviceFactory::ApplyToGlobalState: {e}"))?;
        Ok(factory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdk_version_from_bytes_missing_export() {
        // A minimally-valid PE64 with an empty export table should fail gracefully.
        let result = read_sdk_version_from_bytes(b"MZ");
        assert!(result.is_err(), "expected error for truncated data");
    }

    #[test]
    fn path_cstring_adds_backslash() {
        let p = PathBuf::from(r"C:\test\D3D12");
        let c = sdk_path_cstring(&p).unwrap();
        assert!(c.to_str().unwrap().ends_with('\\'));
    }

    #[test]
    fn path_cstring_keeps_trailing_backslash() {
        let p = PathBuf::from(r"C:\test\D3D12\");
        let c = sdk_path_cstring(&p).unwrap();
        assert_eq!(c.to_str().unwrap().matches('\\').count(), 3);
    }
}
