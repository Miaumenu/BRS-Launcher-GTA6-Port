//! The protection the title asks for each region of its memory
//! (`XMemVirtualAlloc`), kept so the memory accounting can split the
//! mirrored windows and the card copies by what the title meant each
//! region to be. Memory the CPU writes but never reads (write-combined or
//! uncached) can live on the card without ever being copied back, while
//! truly shared memory cannot. See the AMD study, sections 10, 11.5 and
//! 14.2. Measurement only: nothing here changes where memory is placed.

use std::collections::BTreeMap;
use std::sync::RwLock;

/// Title start address -> (len, protection) of each region the title
/// allocated with a protection. The title asks for memory in hundreds of
/// such regions, so the map stays small.
static REGIONS: RwLock<BTreeMap<usize, (usize, u32)>> = RwLock::new(BTreeMap::new());

/// How the memory is split, by the protection the title asked for it. The
/// bit meanings are the title's console protection (see `note_protection`
/// in `gambiarra-hle`'s `xmem`): `0x200` uncached, `0x400` write-combined,
/// `0x2000` reachable by the GPU while cacheable by the CPU. The labels
/// are read from those bits; [`raw_by_class`] prints the exact values
/// behind each class so the reading can be checked against the title.
#[cfg(feature = "diagnostics")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    /// Cacheable CPU memory with no GPU bit: the title's own logic.
    CpuOnly,
    /// Write-combined or uncached: the CPU writes, the GPU reads, and the
    /// CPU never reads it back. Could live on the card with no copy back.
    WriteOnlyGpu,
    /// Cacheable and reachable by the GPU: both sides read and write, so
    /// the copy back is needed.
    Shared,
    /// No protection was recorded for the address.
    Unknown,
}

#[cfg(feature = "diagnostics")]
impl Class {
    pub(crate) const COUNT: usize = 4;

    pub(crate) fn index(self) -> usize {
        match self {
            Class::CpuOnly => 0,
            Class::WriteOnlyGpu => 1,
            Class::Shared => 2,
            Class::Unknown => 3,
        }
    }

    /// Short name for the accounting line, in the order of [`index`].
    pub(crate) fn names() -> [&'static str; Self::COUNT] {
        [
            "cpu-only",
            "wc/uncached (CPU won't read back)",
            "shared",
            "unknown",
        ]
    }
}

/// The title asked the region `[at, at + len)` to have protection
/// `protect`. Called from the HLE when the title allocates memory
/// (`XMemVirtualAlloc`).
pub fn title_memory_protection(at: usize, len: usize, protect: u32) {
    if len == 0 || at == 0 {
        return;
    }
    REGIONS
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .insert(at, (len, protect));
}

/// The region at `at` was freed: dropped so a later region reusing the
/// address is not read as the old one.
pub fn title_memory_protection_gone(at: usize) {
    REGIONS
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&at);
}

/// The protection the title asked for the region that holds `at`, if any.
#[cfg(feature = "diagnostics")]
fn protection(at: usize) -> Option<u32> {
    let regions = REGIONS.read().unwrap_or_else(|e| e.into_inner());
    let (&start, &(len, protect)) = regions.range(..=at).next_back()?;
    (at < start.saturating_add(len)).then_some(protect)
}

/// The class of the memory at title address `at`.
#[cfg(feature = "diagnostics")]
pub(crate) fn classify(at: u64) -> Class {
    match protection(at as usize) {
        None => Class::Unknown,
        // Write-combined (0x400) or uncached (0x200): the CPU does not read
        // it, so what shaders write there need not be found and put back.
        Some(p) if p & 0x600 != 0 => Class::WriteOnlyGpu,
        // Reachable by the GPU and cacheable by the CPU.
        Some(p) if p & 0x2000 != 0 => Class::Shared,
        // Plain cacheable CPU memory.
        Some(_) => Class::CpuOnly,
    }
}

/// For the accounting line: the exact protection values seen, grouped by
/// the class they fall in, as `(class index, protect, bytes)` sorted by
/// bytes. Lets a reader confirm the bit reading above against the title.
#[cfg(feature = "diagnostics")]
pub(crate) fn raw_by_class() -> Vec<(usize, u32, u64)> {
    let regions = REGIONS.read().unwrap_or_else(|e| e.into_inner());
    let mut by: BTreeMap<u32, u64> = BTreeMap::new();
    for (_, &(len, protect)) in regions.iter() {
        *by.entry(protect).or_default() += len as u64;
    }
    let mut out: Vec<(usize, u32, u64)> = by
        .into_iter()
        .map(|(protect, bytes)| {
            let class = match protect {
                p if p & 0x600 != 0 => Class::WriteOnlyGpu,
                p if p & 0x2000 != 0 => Class::Shared,
                _ => Class::CpuOnly,
            };
            (class.index(), protect, bytes)
        })
        .collect();
    out.sort_by_key(|&(_, _, bytes)| std::cmp::Reverse(bytes));
    out
}
