//! Loading a prepared image as a real Windows module and binding it.

use std::collections::HashSet;
use std::ffi::{CString, c_void};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use anyhow::{Context, Result, bail};
use gambiarra_pe::{ImageInfo, ImportSymbol};
use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryExW};
use windows_sys::Win32::System::Memory::{PAGE_READWRITE, VirtualProtect};

use crate::thunks::{ImportSite, ThunkBlock, UnimplementedPolicy};
use crate::{
    DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, ImportResolver, PreparedImage, Provenance, Resolution,
    prepare,
};

/// A title image mapped into this process. A clone names the same mapping
/// (no reference of its own): unload one copy only.
#[derive(Debug, Clone)]
pub struct LoadedImage {
    module: HMODULE,
    path: PathBuf,
    info: ImageInfo,
}

// SAFETY: HMODULE is a process-wide handle (the base address); using it from
// any thread is fine.
unsafe impl Send for LoadedImage {}
// SAFETY: as above; `LoadedImage` has no interior mutability.
unsafe impl Sync for LoadedImage {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingImport {
    pub dll: String,
    pub symbol: String,
}

/// Outcome of [`LoadedImage::bind_imports`].
#[derive(Debug, Default, Clone)]
pub struct BindReport {
    pub hle: usize,
    pub host: usize,
    pub title: usize,
    pub missing: Vec<MissingImport>,
}

/// Writes the prepared copy into `dir` and maps it with the Windows loader.
///
/// `dir` receives derived game data, so it must be outside the repository.
pub fn load(prepared: &PreparedImage, dir: &Path) -> Result<LoadedImage> {
    gambiarra_core::ensure_outside_repo(dir)?;
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join(prepared.file_name());
    write_if_changed(&path, &prepared.bytes)?;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: `wide` is a NUL-terminated path. The image has no entry point
    // and no imports, so loading it runs no title code.
    let module = unsafe { LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    if module.is_null() {
        bail!(
            "LoadLibraryExW({}) failed: {}{}",
            path.display(),
            std::io::Error::last_os_error(),
            if prepared.info.has_relocations {
                ""
            } else {
                " (image has no relocations; its preferred base may be taken)"
            }
        );
    }
    let loaded = LoadedImage {
        module,
        path,
        info: prepared.info.clone(),
    };
    // A DLL indexes its TLS properly; only an exe assumes index 0.
    if !loaded.info.is_dll() {
        // SAFETY: just mapped, and no title code has run.
        unsafe { crate::tls::redirect(loaded.base(), &loaded.info) }?;
        // SAFETY: as above.
        unsafe { render_size(loaded.base(), loaded.info.size_of_image as usize) };
        // SAFETY: as above.
        unsafe { title_patches(loaded.base()) };
    }
    tracing::info!(
        base = format_args!("{:#x}", loaded.base()),
        preferred = format_args!("{:#x}", loaded.info.image_base),
        path = %loaded.path.display(),
        "image mapped"
    );
    crate::threads::mapped(&loaded);
    Ok(loaded)
}

/// `GAMBIARRA_TITLE_PATCH=<rva>:<old hex>:<new hex>[;...]`: bytes of the
/// title's code changed in memory before it runs, each only where the old
/// bytes are found (else the patch is refused and said). An experiment's
/// tool (a size the title renders at, a check it makes); the image file and
/// the title's own files are not touched.
///
/// # Safety
/// `base` is the title image just mapped, before any of its code runs.
unsafe fn title_patches(base: usize) {
    let Ok(list) = std::env::var("GAMBIARRA_TITLE_PATCH") else {
        return;
    };
    let hex = |s: &str| -> Option<Vec<u8>> {
        let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        if s.len() % 2 != 0 {
            return None;
        }
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
            .collect()
    };
    for entry in list.split(';').filter(|e| !e.trim().is_empty()) {
        let parts: Vec<&str> = entry.split(':').collect();
        let parsed = match parts.as_slice() {
            [rva, old, new] => usize::from_str_radix(rva.trim().trim_start_matches("0x"), 16)
                .ok()
                .zip(hex(old))
                .zip(hex(new))
                .map(|((r, o), n)| (r, o, n)),
            _ => None,
        };
        let Some((rva, old, new)) = parsed.filter(|(_, o, n)| o.len() == n.len() && !o.is_empty())
        else {
            tracing::warn!(
                entry,
                "title patch not understood (rva:old:new, as long as each other)"
            );
            continue;
        };
        // SAFETY: the caller's promise.
        if unsafe { holds(base, rva, &old) } {
            // SAFETY: as above, and the old bytes are there.
            unsafe { write_patch(base, rva, &new) };
            tracing::info!(
                rva = format_args!("{rva:#x}"),
                bytes = new.len(),
                "title patched (GAMBIARRA_TITLE_PATCH)"
            );
        }
    }
}

/// Whether the title holds `old` at `rva` (said when not).
///
/// # Safety
/// `base` is the title image just mapped and `rva..rva + old.len()` is inside it.
unsafe fn holds(base: usize, rva: usize, old: &[u8]) -> bool {
    // SAFETY: inside the mapped image (the caller's promise); read only.
    let found = unsafe { std::slice::from_raw_parts((base + rva) as *const u8, old.len()) };
    if found != old {
        tracing::warn!(
            rva = format_args!("{rva:#x}"),
            found = %found.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            "title patch refused: the old bytes are not there"
        );
    }
    found == old
}

/// Writes `new` over the title's bytes at `rva`.
///
/// # Safety
/// As [`holds`], and no title code runs yet.
unsafe fn write_patch(base: usize, rva: usize, new: &[u8]) {
    let at = base + rva;
    let mut protection = 0;
    // SAFETY: making our own mapping writable for a moment.
    if unsafe {
        VirtualProtect(
            at as _,
            new.len(),
            windows_sys::Win32::System::Memory::PAGE_EXECUTE_READWRITE,
            &mut protection,
        )
    } == 0
    {
        tracing::warn!(
            rva = format_args!("{rva:#x}"),
            "title patch: VirtualProtect failed"
        );
        return;
    }
    // SAFETY: writable now; no title code runs yet.
    unsafe {
        std::ptr::copy_nonoverlapping(new.as_ptr(), at as *mut u8, new.len());
        VirtualProtect(at as _, new.len(), protection, &mut protection);
        windows_sys::Win32::System::Diagnostics::Debug::FlushInstructionCache(
            windows_sys::Win32::System::Threading::GetCurrentProcess(),
            at as _,
            new.len(),
        );
    }
}

/// `GAMBIARRA_RENDER_SIZE=WxH` (640x360 to 1920x1080, multiples of 8): the
/// size the title renders at inside, below the 1920x1080 it picks on its own.
/// Off unless set. Only for the one build these bytes were read from
/// (`game_scarlett_master.exe`); on another, nothing is written and that is said.
///
/// The title picks the entry of its display-mode table nearest to 1920x1080
/// (`.rdata` +0x615C0C0: 1280x720, 1664x936, 1920x1080, 2560x1440,
/// 3840x2160, each `w, h, 60, 1`), so every entry becomes `WxH`. Two checks
/// follow from rendering smaller, and both are made safe:
/// - +0x247AF2B: a 64 MB arena for the render targets must end with under
///   2 MB unused, else `int3`; smaller targets leave more unused, which is
///   harmless, so the `jbe` over the `int3` becomes a `jmp`.
/// - +0x28F03E8: below its output size the title runs its upscaler's path,
///   which steps a jitter phase `frame % count`; the count is set by the
///   console upscaler DLL, which the host refuses, so it stays 0 (a divide
///   by zero). The divide becomes `frame & 7`: 8 phases, what FSR 2's
///   `8 * (output / render)^2` gives at an even scale.
///
/// # Safety
/// `base` is the title image just mapped, before any of its code runs.
unsafe fn render_size(base: usize, image_size: usize) {
    let Ok(asked) = std::env::var("GAMBIARRA_RENDER_SIZE") else {
        return;
    };
    let size = asked
        .trim()
        .split_once(['x', 'X'])
        .and_then(|(w, h)| Some((w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?)));
    let Some((w, h)) = size.filter(|&(w, h)| {
        (640..=1920).contains(&w)
            && (360..=1080).contains(&h)
            && w.is_multiple_of(8)
            && h.is_multiple_of(8)
    }) else {
        tracing::warn!(
            asked,
            "GAMBIARRA_RENDER_SIZE not taken (WxH, 640x360 to 1920x1080, multiples of 8)"
        );
        return;
    };
    let pair = |w: u32, h: u32| [w.to_le_bytes(), h.to_le_bytes()].concat();
    let mut patches: Vec<(usize, Vec<u8>, Vec<u8>)> = [
        (1280, 720),
        (1664, 936),
        (1920, 1080),
        (2560, 1440),
        (3840, 2160),
    ]
    .iter()
    .enumerate()
    .map(|(i, &(ow, oh))| (0x615_c0c0 + i * 16, pair(ow, oh), pair(w, h)))
    .collect();
    patches.push((0x247_af2b, vec![0x76, 0x01, 0xcc], vec![0xeb, 0x01, 0xcc]));
    // div dword ptr [rdi+0x10c] -> and eax, 7; mov edx, eax; nop
    patches.push((
        0x28f_03e8,
        vec![0xf7, 0xb7, 0x0c, 0x01, 0x00, 0x00],
        vec![0x83, 0xe0, 0x07, 0x89, 0xc2, 0x90],
    ));
    // All or nothing: half of these would leave the title asserting.
    // SAFETY: the caller's promise; each range is checked to be inside the
    // image first, and `holds` only reads.
    if !patches
        .iter()
        .all(|(rva, old, _)| rva + old.len() <= image_size && unsafe { holds(base, *rva, old) })
    {
        tracing::warn!(
            w,
            h,
            "GAMBIARRA_RENDER_SIZE not applied: not the build its bytes were read from"
        );
        return;
    }
    for (rva, _, new) in &patches {
        // SAFETY: as above; the old bytes were all found.
        unsafe { write_patch(base, *rva, new) };
    }
    tracing::info!(
        w,
        h,
        "title renders at this size inside (GAMBIARRA_RENDER_SIZE)"
    );
}

/// Maps the DLLs the title ships in `title_dir` that `main` imports,
/// directly or through each other, in initialisation order (dependencies
/// first). Only DLL names `wanted` accepts are considered, so DLLs the host
/// or our runtime provide stay with them. Imports are not bound.
pub fn load_title_dlls(
    main: &ImageInfo,
    title_dir: &Path,
    cache_dir: &Path,
    wanted: &dyn Fn(&str) -> bool,
) -> Result<Vec<(String, LoadedImage)>> {
    let mut loaded = Vec::new();
    let dirs = (title_dir, cache_dir);
    map_imports(main, dirs, wanted, &mut HashSet::new(), &mut loaded)?;
    Ok(loaded)
}

/// Maps the DLL `name` from `title_dir` like [`load_title_dlls`] does, with
/// the title DLLs it imports (dependencies first, `name` last); empty if
/// `wanted` refuses it or the folder has no such file. For DLLs a title
/// loads itself; `wanted` must refuse those already mapped.
pub fn load_title_dll(
    name: &str,
    title_dir: &Path,
    cache_dir: &Path,
    wanted: &dyn Fn(&str) -> bool,
) -> Result<Vec<(String, LoadedImage)>> {
    let mut loaded = Vec::new();
    let dirs = (title_dir, cache_dir);
    map_dll(name, dirs, wanted, &mut HashSet::new(), &mut loaded)?;
    Ok(loaded)
}

fn map_imports(
    info: &ImageInfo,
    dirs: (&Path, &Path),
    wanted: &dyn Fn(&str) -> bool,
    seen: &mut HashSet<String>,
    loaded: &mut Vec<(String, LoadedImage)>,
) -> Result<()> {
    for module in info.imports.iter().chain(&info.delay_imports) {
        map_dll(&module.dll, dirs, wanted, seen, loaded)?;
    }
    Ok(())
}

fn map_dll(
    name: &str,
    dirs: (&Path, &Path),
    wanted: &dyn Fn(&str) -> bool,
    seen: &mut HashSet<String>,
    loaded: &mut Vec<(String, LoadedImage)>,
) -> Result<()> {
    let name = name.to_ascii_lowercase();
    let path = dirs.0.join(&name);
    if !seen.insert(name.clone()) || !wanted(&name) || !path.is_file() {
        return Ok(());
    }
    let original = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let prepared = prepare(&original).with_context(|| format!("preparing {}", path.display()))?;
    let image = load(&prepared, dirs.1).with_context(|| format!("loading {}", path.display()))?;
    map_imports(&image.info, dirs, wanted, seen, loaded)?;
    loaded.push((name, image));
    Ok(())
}

/// Resolves through `inner`, then through the exports of the title's DLLs.
pub struct WithTitleDlls<'a> {
    pub inner: &'a dyn ImportResolver,
    pub dlls: &'a [(String, LoadedImage)],
}

impl ImportResolver for WithTitleDlls<'_> {
    fn resolve(&self, dll: &str, symbol: &ImportSymbol) -> Resolution {
        let found = self.inner.resolve(dll, symbol);
        if found != Resolution::Missing {
            return found;
        }
        self.dlls
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(dll))
            .and_then(|(_, image)| image.export_symbol(symbol))
            .map_or(Resolution::Missing, |address| Resolution::Found {
                address,
                provenance: Provenance::Title,
            })
    }
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<()> {
    crate::publish::write_if_changed(path, bytes)
        .with_context(|| format!("publishing {}", path.display()))
}

impl LoadedImage {
    pub fn base(&self) -> usize {
        self.module as usize
    }

    /// Parsed from the original image (imports, entry point, TLS callbacks).
    pub fn info(&self) -> &ImageInfo {
        &self.info
    }

    /// Address of an export of the image, if it has one by that name.
    pub fn export(&self, name: &str) -> Option<usize> {
        let name = CString::new(name).ok()?;
        // SAFETY: valid module handle and NUL-terminated name.
        unsafe { GetProcAddress(self.module, name.as_ptr().cast()) }
            .map(|f| f as *const () as usize)
    }

    /// Address of an export, by name or ordinal.
    pub fn export_symbol(&self, symbol: &ImportSymbol) -> Option<usize> {
        match symbol {
            ImportSymbol::Name { name, .. } => self.export(name),
            // SAFETY: MAKEINTRESOURCE-style ordinal lookup on a valid module.
            ImportSymbol::Ordinal(o) => unsafe { GetProcAddress(self.module, *o as usize as _) }
                .map(|f| f as *const () as usize),
        }
    }

    /// Fills every IAT slot of the image.
    ///
    /// Each import goes to `resolver`; whatever it cannot resolve gets an
    /// unimplemented-import stub that behaves according to `policy`.
    pub fn bind_imports(
        &self,
        resolver: &dyn ImportResolver,
        policy: UnimplementedPolicy,
    ) -> Result<BindReport> {
        let mut report = BindReport::default();
        let mut slots: Vec<(u32, usize)> = Vec::with_capacity(self.info.import_count());
        let mut unresolved: Vec<(u32, &str, &ImportSymbol)> = Vec::new();

        // Delay-load slots are bound eagerly too, so the title's own
        // delay-load helper (which would ask the host) never runs.
        for module in self.info.imports.iter().chain(&self.info.delay_imports) {
            for entry in &module.entries {
                match resolver.resolve(&module.dll, &entry.symbol) {
                    Resolution::Found {
                        address,
                        provenance,
                    } => {
                        match provenance {
                            Provenance::Hle => report.hle += 1,
                            Provenance::Host => report.host += 1,
                            Provenance::Title => report.title += 1,
                        }
                        slots.push((entry.iat_slot_rva, address));
                    }
                    Resolution::Missing => {
                        unresolved.push((entry.iat_slot_rva, &module.dll, &entry.symbol));
                    }
                }
            }
        }

        if !unresolved.is_empty() {
            let mut block = ThunkBlock::new(unresolved.len())?;
            for (slot, dll, symbol) in unresolved {
                let site: &'static ImportSite = Box::leak(Box::new(ImportSite {
                    dll: dll.to_owned(),
                    symbol: symbol.to_string(),
                    policy,
                    calls: AtomicU64::new(0),
                }));
                slots.push((slot, block.emit(site)?));
                report.missing.push(MissingImport {
                    dll: site.dll.clone(),
                    symbol: site.symbol.clone(),
                });
            }
            block.seal()?;
        }

        self.write_slots(&slots)?;
        tracing::info!(
            hle = report.hle,
            host = report.host,
            missing = report.missing.len(),
            "imports bound"
        );
        Ok(report)
    }

    /// Protects and restores each slot on its own: import tables can sit in
    /// sections with different protections (delay-load tables in `.didat`).
    fn write_slots(&self, slots: &[(u32, usize)]) -> Result<()> {
        for &(rva, address) in slots {
            let slot = (self.base() + rva as usize) as *mut u64;
            let mut old = 0;
            // SAFETY: the parser only yields slots inside the image.
            if unsafe { VirtualProtect(slot as *const c_void, 8, PAGE_READWRITE, &mut old) } == 0 {
                bail!(
                    "VirtualProtect(IAT slot {rva:#x}) failed: {}",
                    std::io::Error::last_os_error()
                );
            }
            // SAFETY: inside the image, now writable, 8-byte aligned by the PE format.
            unsafe { slot.write_volatile(address as u64) };
            let mut ignored = 0;
            // SAFETY: restoring the protection we just changed.
            unsafe { VirtualProtect(slot as *const c_void, 8, old, &mut ignored) };
        }
        Ok(())
    }

    /// Calls the image's TLS callbacks with `reason` (e.g. `DLL_PROCESS_ATTACH`).
    ///
    /// # Safety
    /// Runs title code. Imports must be bound first.
    pub unsafe fn run_tls_callbacks(&self, reason: u32) {
        // SAFETY: forwarded from the caller.
        unsafe { self.tls_callbacks(reason, std::ptr::null_mut()) }
    }

    /// # Safety
    /// See [`Self::run_tls_callbacks`].
    unsafe fn tls_callbacks(&self, reason: u32, reserved: *mut c_void) {
        let Some(tls) = &self.info.tls else { return };
        for &rva in &tls.callback_rvas {
            type TlsCallback = unsafe extern "system" fn(*mut c_void, u32, *mut c_void);
            // SAFETY: the RVA came from the image's own TLS directory.
            let callback: TlsCallback = unsafe { std::mem::transmute(self.base() + rva as usize) };
            // SAFETY: the caller guarantees imports are bound.
            unsafe { callback(self.module, reason, reserved) };
        }
    }

    /// Initialises a DLL as the Windows loader would: its TLS callbacks,
    /// then `DllMain(DLL_PROCESS_ATTACH)`. From then on it hears of every
    /// thread start and end (see `threads`).
    ///
    /// # Safety
    /// Runs title code. Imports must be bound first, and the DLLs this one
    /// depends on attached.
    pub unsafe fn attach(&self) -> Result<()> {
        // SAFETY: forwarded from the caller.
        if unsafe { self.notify(DLL_PROCESS_ATTACH, true, std::ptr::null_mut()) } == Some(0) {
            bail!("DllMain returned FALSE");
        }
        crate::threads::attached(self.base());
        Ok(())
    }

    /// `DLL_PROCESS_DETACH` as `FreeLibrary` sends it (TLS callbacks, then
    /// `DllMain`); the image hears of no more threads.
    ///
    /// # Safety
    /// Runs title code. The image must have been attached (or its attach
    /// failed), and no other thread may be using it.
    pub unsafe fn detach(&self) {
        crate::threads::detached(self.base());
        // SAFETY: forwarded from the caller.
        unsafe { self.notify(DLL_PROCESS_DETACH, true, std::ptr::null_mut()) };
    }

    /// Runs the image's TLS callbacks with `reason`, then, if `dll_main` and
    /// the image is a DLL with an entry point, its `DllMain`, whose result
    /// is returned. `reserved` is the entry points' third argument.
    ///
    /// # Safety
    /// Runs title code. Imports must be bound first.
    pub(crate) unsafe fn notify(
        &self,
        reason: u32,
        dll_main: bool,
        reserved: *mut c_void,
    ) -> Option<i32> {
        // SAFETY: forwarded from the caller.
        unsafe { self.tls_callbacks(reason, reserved) };
        if !dll_main || !self.info.is_dll() || self.info.entry_rva == 0 {
            return None;
        }
        type DllMain = unsafe extern "system" fn(*mut c_void, u32, *mut c_void) -> i32;
        // SAFETY: the entry RVA came from the original image header.
        let main: DllMain =
            unsafe { std::mem::transmute(self.base() + self.info.entry_rva as usize) };
        // SAFETY: forwarded from the caller.
        Some(unsafe { main(self.module, reason, reserved) })
    }

    /// Starts the title: on a new thread with the stack size from the image
    /// header, attaches `dlls` in order (see [`Self::attach`]), runs the
    /// image's TLS callbacks (`DLL_PROCESS_ATTACH`), then its original entry
    /// point. Returns the thread so the caller can join it.
    ///
    /// # Safety
    /// Runs title code. Imports of the image and of `dlls` must be bound.
    pub unsafe fn spawn_main(
        self: Arc<Self>,
        dlls: Vec<(String, LoadedImage)>,
    ) -> Result<std::thread::JoinHandle<Result<u32>>> {
        if self.info.entry_rva == 0 {
            bail!("image has no entry point");
        }
        let stack = usize::try_from(self.info.stack_reserve)
            .unwrap_or(usize::MAX)
            .max(1 << 20);
        std::thread::Builder::new()
            .name("title-main".into())
            .stack_size(stack)
            .spawn(move || {
                for (name, dll) in &dlls {
                    // SAFETY: forwarded from the caller of `spawn_main`.
                    unsafe { dll.attach() }.with_context(|| format!("attaching {name}"))?;
                }
                // SAFETY: forwarded from the caller of `spawn_main`.
                unsafe { self.run_tls_callbacks(DLL_PROCESS_ATTACH) };
                crate::threads::attached(self.base());
                type Entry = unsafe extern "system" fn(*mut c_void) -> u32;
                // SAFETY: the entry RVA came from the original image header.
                let entry: Entry =
                    unsafe { std::mem::transmute(self.base() + self.info.entry_rva as usize) };
                // SAFETY: forwarded from the caller of `spawn_main`.
                Ok(unsafe { entry(std::ptr::null_mut()) })
            })
            .context("spawning title main thread")
    }

    /// Unmaps the image.
    ///
    /// # Safety
    /// No title code may be running or run later, and no pointer into the
    /// image may be used afterwards.
    pub unsafe fn unload(self) {
        crate::threads::unmapped(self.base());
        // SAFETY: guaranteed by the caller.
        unsafe { FreeLibrary(self.module) };
    }
}
