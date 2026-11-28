//! Passes given a neutral result instead of being run (`GAMBIARRA_NEUTRAL_PASSES`).
//!
//! On a weak card some of the title's compute passes cost more than the rest of the frame
//! (the raymarched local lights, the classified lighting gathers, the clouds). Skipping one
//! (`GAMBIARRA_SKIP_PASSES`) hangs the card when a later pass reads a count, the arguments
//! of an indirect dispatch or an append buffer the skipped one should have written. Here a
//! pass is replaced only when its console code stores to no buffer at all
//! ([`crate::pipeline::Translated::stores`]: no buffer store, buffer atomic or scalar store;
//! image stores only), so nothing downstream can wait on a counter it never wrote; a pass
//! that does store to buffers keeps running as the title has it (logged once).
//!
//! What the replaced pass leaves in the textures it writes is chosen per pass:
//! - `keep` (the default): nothing is written. For a pass that adds its light to images other
//!   passes wrote first (the gathers, the local lights) that is "no light added".
//! - a value: every texture it writes through a view is filled with it (clouds:
//!   `0:0:0:1`, no scattering and full transmittance). Integer formats get the value
//!   converted to integers.
//!
//! `GAMBIARRA_NEUTRAL_PASSES=<entry>[,<entry>...]`, an entry
//! `<target>[@<slot>+<offset>][=<fill>]`:
//! - `target`: a 16-digit shader hash as the logs print it, or a part of the program's name
//!   (`CS_GatherRaymarchedLocalLights`, `Cloud`; at least 3 characters, matched in the
//!   container as `GAMBIARRA_SKIP_PASSES` matches).
//! - `@<slot>+<offset>`: only that view (root parameter and dwords into its table, as the
//!   frame listings print `Uav s1+64`); without it the fill is the pass's for all its views.
//! - `fill`: `keep`, `zero` (0:0:0:0), `one` (1:1:1:1), or one to four numbers separated by
//!   `:` (one number fills all four channels; fewer than four leave the rest 0, alpha 1).
//!
//! Only compute passes, direct or indirect dispatches; off unless the variable is set.

use std::sync::{Arc, Mutex, OnceLock};

use windows::Win32::Graphics::Direct3D12::*;
use windows::core::s;

use crate::host::{HostList, host};

/// What a view of a replaced pass is left holding.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Fill {
    /// Not written.
    Keep,
    /// Filled with this value.
    Value([f32; 4]),
}

/// One entry of the setting.
#[derive(Clone, PartialEq, Debug)]
struct Entry {
    /// A shader hash (lower case) or a part of a program name.
    target: Target,
    /// One view only: root parameter, dwords into its table.
    at: Option<(u8, u32)>,
    fill: Fill,
}

#[derive(Clone, PartialEq, Debug)]
enum Target {
    Hash(String),
    Name(Vec<u8>),
}

/// What a shader the setting names gets.
#[derive(Debug, PartialEq)]
pub(crate) struct Rule {
    /// The entries that named it, as written, for the logs.
    pub(crate) names: String,
    /// For every view without one of its own.
    pub(crate) all: Fill,
    /// Views with their own.
    pub(crate) at: Vec<((u8, u32), Fill)>,
}

impl Rule {
    /// The fill of the view at `slot`, `offset`.
    pub(crate) fn fill(&self, slot: u8, offset: u32) -> Fill {
        self.at
            .iter()
            .rev()
            .find(|(at, _)| *at == (slot, offset))
            .map_or(self.all, |&(_, fill)| fill)
    }

    /// Whether anything is written for the pass.
    pub(crate) fn writes(&self) -> bool {
        self.all != Fill::Keep || self.at.iter().any(|(_, f)| *f != Fill::Keep)
    }
}

fn parse_fill(text: &str) -> Option<Fill> {
    let text = text.trim();
    match text {
        "" | "keep" => return Some(Fill::Keep),
        "zero" => return Some(Fill::Value([0.0; 4])),
        "one" => return Some(Fill::Value([1.0; 4])),
        _ => {}
    }
    let numbers: Vec<f32> = text
        .split(':')
        .map(|n| n.trim().parse::<f32>().ok().filter(|v| v.is_finite()))
        .collect::<Option<_>>()?;
    match numbers[..] {
        [v] => Some(Fill::Value([v; 4])),
        [r, g] => Some(Fill::Value([r, g, 0.0, 1.0])),
        [r, g, b] => Some(Fill::Value([r, g, b, 1.0])),
        [r, g, b, a] => Some(Fill::Value([r, g, b, a])),
        _ => None,
    }
}

fn parse_entry(text: &str) -> Option<Entry> {
    let (head, fill) = match text.split_once('=') {
        Some((head, fill)) => (head, parse_fill(fill)?),
        None => (text, Fill::Keep),
    };
    let (target, at) = match head.split_once('@') {
        Some((target, at)) => {
            let (slot, offset) = at.trim().trim_start_matches('s').split_once('+')?;
            (
                target,
                Some((slot.trim().parse().ok()?, offset.trim().parse().ok()?)),
            )
        }
        None => (head, None),
    };
    let target = target.trim();
    let target = if target.len() == 16 && target.bytes().all(|b| b.is_ascii_hexdigit()) {
        Target::Hash(target.to_ascii_lowercase())
    } else if target.len() >= 3 {
        Target::Name(target.as_bytes().to_vec())
    } else {
        return None;
    };
    Some(Entry { target, at, fill })
}

fn parse(value: &str) -> Vec<Entry> {
    value
        .split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .filter_map(|e| {
            let entry = parse_entry(e);
            if entry.is_none() {
                tracing::warn!(
                    entry = e,
                    "GAMBIARRA_NEUTRAL_PASSES: not understood (<hash or name part>[@<slot>+<offset>][=keep|zero|one|r:g:b:a])"
                );
            }
            entry
        })
        .collect()
}

fn entries() -> &'static [Entry] {
    static ENTRIES: OnceLock<Vec<Entry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        let value = std::env::var("GAMBIARRA_NEUTRAL_PASSES").unwrap_or_default();
        let entries = parse(&value);
        if !entries.is_empty() {
            tracing::info!(passes = %value, "passes given a neutral result (GAMBIARRA_NEUTRAL_PASSES)");
        }
        entries
    })
}

/// Whether the setting names anything.
pub(crate) fn on() -> bool {
    !entries().is_empty()
}

/// The rule for a shader of `stage`, `hash` and `container`; `None` if no entry names it.
pub(crate) fn rule_for(stage: &str, hash: &str, container: &[u8]) -> Option<Arc<Rule>> {
    rule_of(entries(), stage, hash, container).map(Arc::new)
}

fn rule_of(entries: &[Entry], stage: &str, hash: &str, container: &[u8]) -> Option<Rule> {
    if stage != "compute" {
        return None;
    }
    let mut rule: Option<Rule> = None;
    for e in entries {
        let named = match &e.target {
            Target::Hash(h) => h == hash,
            Target::Name(part) => container.windows(part.len()).any(|w| w == &part[..]),
        };
        if !named {
            continue;
        }
        let r = rule.get_or_insert_with(|| Rule {
            names: String::new(),
            all: Fill::Keep,
            at: Vec::new(),
        });
        if !r.names.is_empty() {
            r.names.push(',');
        }
        r.names.push_str(&match &e.target {
            Target::Hash(h) => h.clone(),
            Target::Name(n) => String::from_utf8_lossy(n).into_owned(),
        });
        match e.at {
            Some(at) => r.at.push((at, e.fill)),
            None => r.all = e.fill,
        }
    }
    rule
}

/// The title's name of the program in a container (`CS_CloudMap` and the like, without the
/// compiler's hash suffix), if it carries one.
pub(crate) fn program_name(container: &[u8]) -> Option<String> {
    const PREFIXES: [&[u8]; 8] = [b"CS_", b"PS_", b"VS_", b"MS_", b"AS_", b"GS_", b"HS_", b"DS_"];
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    for i in 0..container.len().saturating_sub(6) {
        if i > 0 && word(container[i - 1]) {
            continue;
        }
        if !PREFIXES.iter().any(|p| container[i..].starts_with(p)) {
            continue;
        }
        let len = container[i..].iter().take_while(|&&b| word(b)).count();
        if !(6..=83).contains(&len) {
            continue;
        }
        let mut name = String::from_utf8_lossy(&container[i..i + len]).into_owned();
        if let Some(stem) = name.strip_suffix("_Wrapped") {
            name.truncate(stem.len());
        }
        // The compiler's hash: `_` and eight hex digits.
        let bytes = name.as_bytes();
        if bytes.len() > 9
            && bytes[bytes.len() - 9] == b'_'
            && bytes[bytes.len() - 8..].iter().all(u8::is_ascii_hexdigit)
        {
            name.truncate(bytes.len() - 9);
        }
        return Some(name);
    }
    None
}

/// Says once per shader what it gets: replaced, or run as the title has it because it stores
/// to buffers.
pub(crate) fn note(hash: &str, container: &[u8], rule: &Rule, stores: bool) {
    static SEEN: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if !seen
        .get_or_insert_with(Default::default)
        .insert(hash.to_owned())
    {
        return;
    }
    let program = program_name(container).unwrap_or_default();
    if stores {
        tracing::warn!(
            shader = hash,
            %program,
            named = %rule.names,
            "neutral pass not applied: its code stores to buffers (counts, arguments, lists others may read); it runs as the title has it"
        );
    } else {
        tracing::info!(
            shader = hash,
            %program,
            named = %rule.names,
            all = ?rule.all,
            views = ?rule.at,
            "neutral pass: replaced"
        );
    }
}

/// How a view is filled: its texture's shape and the kind of value it takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Shape {
    Plain,
    Array,
    Volume,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Float,
    Uint,
    Sint,
}

/// Of a DXGI format: whether its values are unsigned or signed integers, or else floats
/// (float, unorm, snorm); `None` for typeless formats.
pub(crate) fn kind_of(format: u32) -> Option<Kind> {
    match format {
        3 | 7 | 12 | 17 | 25 | 30 | 36 | 42 | 50 | 57 | 62 => Some(Kind::Uint),
        4 | 8 | 14 | 18 | 32 | 38 | 43 | 52 | 59 | 64 => Some(Kind::Sint),
        // Typeless.
        1 | 5 | 9 | 15 | 19 | 23 | 27 | 33 | 39 | 44 | 48 | 53 | 60 | 70 | 73 | 76 | 79 | 82
        | 90 | 92 | 94 | 97 => None,
        0 => None,
        _ => Some(Kind::Float),
    }
}

/// A view to fill.
pub(crate) struct ToFill {
    pub(crate) view: D3D12_GPU_DESCRIPTOR_HANDLE,
    pub(crate) shape: Shape,
    pub(crate) kind: Kind,
    /// Width, height, depth or slices.
    pub(crate) size: [u32; 3],
    pub(crate) value: [f32; 4],
}

const SOURCE: &str = r#"
#define RS "RootConstants(num32BitConstants=8, b0), DescriptorTable(UAV(u0))"
cbuffer Set : register(b0) { float4 value; uint4 size; };
#if defined(UINT)
#define T uint4
#define V uint4(max(value, 0.0))
#elif defined(SINT)
#define T int4
#define V int4(value)
#else
#define T float4
#define V value
#endif
#if defined(VOLUME)
RWTexture3D<T> o : register(u0);
#elif defined(ARRAY)
RWTexture2DArray<T> o : register(u0);
#else
RWTexture2D<T> o : register(u0);
#endif
[RootSignature(RS)]
[numthreads(8, 8, 1)]
void fill(uint3 at : SV_DispatchThreadID) {
    if (at.x >= size.x || at.y >= size.y || at.z >= size.z) return;
#if defined(VOLUME) || defined(ARRAY)
    o[at] = V;
#else
    o[at.xy] = V;
#endif
}
"#;

struct Pass {
    root: ID3D12RootSignature,
    /// By kind (float, uint, sint), then shape (plain, array, volume).
    fill: [[ID3D12PipelineState; 3]; 3],
}
// SAFETY: D3D12 objects are free-threaded.
unsafe impl Send for Pass {}
// SAFETY: as above.
unsafe impl Sync for Pass {}

fn make(device: &ID3D12Device) -> windows::core::Result<Pass> {
    let mut root: Option<ID3D12RootSignature> = None;
    let mut pipeline = |kind: &str, shape: &str| -> windows::core::Result<ID3D12PipelineState> {
        let code = crate::host::compile(
            &format!("#define {kind}\n#define {shape}\n{SOURCE}"),
            s!("fill"),
            s!("cs_5_1"),
        )?;
        // SAFETY: the root signature is inside the shader; plain creations.
        unsafe {
            if root.is_none() {
                root = Some(device.CreateRootSignature(0, &code)?);
            }
            let desc = D3D12_COMPUTE_PIPELINE_STATE_DESC {
                pRootSignature: std::mem::transmute_copy(&root),
                CS: D3D12_SHADER_BYTECODE {
                    pShaderBytecode: code.as_ptr().cast(),
                    BytecodeLength: code.len(),
                },
                ..Default::default()
            };
            device.CreateComputePipelineState(&desc)
        }
    };
    let mut row = |kind| -> windows::core::Result<[ID3D12PipelineState; 3]> {
        Ok([
            pipeline(kind, "PLAIN")?,
            pipeline(kind, "ARRAY")?,
            pipeline(kind, "VOLUME")?,
        ])
    };
    let fill = [row("FLOAT")?, row("UINT")?, row("SINT")?];
    Ok(Pass {
        root: root.ok_or_else(windows::core::Error::empty)?,
        fill,
    })
}

fn pass() -> Option<&'static Pass> {
    static PASS: OnceLock<Option<Pass>> = OnceLock::new();
    PASS.get_or_init(|| {
        let host = host()?;
        make(&host.device)
            .inspect_err(|e| tracing::warn!(error = %e, "no neutral passes: the fill does not build"))
            .ok()
    })
    .as_ref()
}

fn uav_barrier(list: &ID3D12GraphicsCommandList) {
    let barrier = D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_UAV,
        ..Default::default()
    };
    // SAFETY: an open list; the barrier names no resource.
    unsafe { list.ResourceBarrier(&[barrier]) };
}

/// Fills `targets` in `list`, whose view heap holds them and is set. Replaces the list's
/// compute root signature and pipeline; false if it recorded nothing.
pub(crate) fn fill(list: &HostList, targets: &[ToFill]) -> bool {
    if targets.is_empty() {
        return false;
    }
    let Some(pass) = pass() else {
        return false;
    };
    let l = &list.list;
    // SAFETY: an open list with the views' heap set; the title's program would have written
    // these views here, so their textures are in the unordered access state (its bind put
    // them there).
    unsafe {
        uav_barrier(l);
        l.SetComputeRootSignature(&pass.root);
        for t in targets {
            let mut words = [0u32; 8];
            for (to, v) in words.iter_mut().zip(t.value) {
                *to = v.to_bits();
            }
            words[4..7].copy_from_slice(&t.size);
            let kind = match t.kind {
                Kind::Float => 0,
                Kind::Uint => 1,
                Kind::Sint => 2,
            };
            let shape = match t.shape {
                Shape::Plain => 0,
                Shape::Array => 1,
                Shape::Volume => 2,
            };
            l.SetPipelineState(&pass.fill[kind][shape]);
            l.SetComputeRoot32BitConstants(0, 8, words.as_ptr().cast(), 0);
            l.SetComputeRootDescriptorTable(1, t.view);
            l.Dispatch(t.size[0].div_ceil(8), t.size[1].div_ceil(8), t.size[2].max(1));
        }
        uav_barrier(l);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_parse() {
        let e = parse(
            "CS_GatherRaymarchedLocalLights, 6697DFF15287E544=zero,Cloud@s1+64=0:0:0:1,x,Cloud=2",
        );
        assert_eq!(e.len(), 4);
        assert_eq!(
            e[0],
            Entry {
                target: Target::Name(b"CS_GatherRaymarchedLocalLights".to_vec()),
                at: None,
                fill: Fill::Keep
            }
        );
        assert_eq!(e[1].target, Target::Hash("6697dff15287e544".into()));
        assert_eq!(e[1].fill, Fill::Value([0.0; 4]));
        assert_eq!(e[2].at, Some((1, 64)));
        assert_eq!(e[2].fill, Fill::Value([0.0, 0.0, 0.0, 1.0]));
        assert_eq!(e[3].fill, Fill::Value([2.0; 4]));
        assert!(parse("Cloud=a:b").is_empty());
        assert_eq!(parse_fill("1:0.5:0"), Some(Fill::Value([1.0, 0.5, 0.0, 1.0])));
    }

    #[test]
    fn rules_by_name_and_hash() {
        let e = parse("Cloud=0:0:0:1,Cloud@1+72=1,0123456789abcdef");
        let r = rule_of(&e, "compute", "ffff", b"..CS_CloudMap_12345678..").unwrap();
        assert_eq!(r.all, Fill::Value([0.0, 0.0, 0.0, 1.0]));
        assert_eq!(r.fill(1, 72), Fill::Value([1.0; 4]));
        assert_eq!(r.fill(1, 64), Fill::Value([0.0, 0.0, 0.0, 1.0]));
        assert!(r.writes());
        let r = rule_of(&e, "compute", "0123456789abcdef", b"CS_Other").unwrap();
        assert_eq!(r.all, Fill::Keep);
        assert!(!r.writes());
        assert!(rule_of(&e, "pixel", "0123456789abcdef", b"").is_none());
        assert!(rule_of(&e, "compute", "1111", b"CS_Other").is_none());
    }

    #[test]
    fn names_from_containers() {
        assert_eq!(
            program_name(b"\0\0CS_GatherRaymarchedLocalLightsClassified16_0f05597d_Wrapped\0").as_deref(),
            Some("CS_GatherRaymarchedLocalLightsClassified16")
        );
        assert_eq!(program_name(b"xCS_Foo\0 PS_Bar_12\0").as_deref(), Some("PS_Bar_12"));
        assert_eq!(program_name(b"nothing"), None);
    }

    #[test]
    fn kinds() {
        assert_eq!(kind_of(42), Some(Kind::Uint));
        assert_eq!(kind_of(10), Some(Kind::Float));
        assert_eq!(kind_of(26), Some(Kind::Float));
        assert_eq!(kind_of(9), None);
        assert_eq!(kind_of(43), Some(Kind::Sint));
    }

    /// The fills compile with the system's compiler and the host takes them.
    #[test]
    fn the_fills_build() {
        let Some(host) = host() else {
            return;
        };
        make(&host.device).expect("the fills");
    }
}
