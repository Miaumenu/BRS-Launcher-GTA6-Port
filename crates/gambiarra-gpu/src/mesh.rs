//! What emulated mesh dispatches store (`pipeline::host::Emulated`, `draw::State::dispatch_mesh`):
//! one buffer on the card that the mesh programs run as compute write and the drawing
//! pipelines read, each dispatch (or part of one) taking the next piece of it in turn.
//! `GAMBIARRA_MESH_SCRATCH_MB` sets its size (64 MiB unless set): a dispatch whose groups
//! do not fit runs in parts.
//!
//! shortcut: pieces are taken round the buffer without asking whether the card is done
//! with the piece taken before; a frame's mesh dispatches together write less than the
//! buffer, so a piece comes round again only frames later.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC;

/// Pieces start on this many bytes (a root view's address need only be 4-aligned; this
/// keeps neighbouring dispatches off each other's cache lines).
const ALIGN: u64 = 256;

pub(crate) struct Scratch {
    pub(crate) buffer: ID3D12Resource,
    /// The buffer's address on the card.
    pub(crate) gpu: u64,
    /// The first half, which dispatches with their group count on the CPU take pieces of
    /// ([`Scratch::take`]); indirect ones take theirs on the card from the second
    /// ([`Scratch::indirect`]).
    pub(crate) bytes: u64,
    next: AtomicU64,
}

impl Scratch {
    /// The second half: its address and size.
    pub(crate) fn indirect(&self) -> (u64, u64) {
        (self.gpu + self.bytes, self.bytes)
    }
}

// SAFETY: a D3D12 resource may be used from any thread; the cursor is atomic.
unsafe impl Send for Scratch {}
// SAFETY: as above.
unsafe impl Sync for Scratch {}

impl Scratch {
    /// The offset of the next `bytes` of the buffer (at most its size).
    pub(crate) fn take(&self, bytes: u64) -> u64 {
        let mut at = 0;
        let _ = self
            .next
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                at = place(next, bytes, self.bytes);
                Some(at + bytes)
            });
        at
    }
}

/// Where `bytes` go in a buffer of `total` bytes whose next free byte is `next`.
fn place(next: u64, bytes: u64, total: u64) -> u64 {
    let at = next.next_multiple_of(ALIGN);
    if at + bytes > total { 0 } else { at }
}

/// The buffer, made on first use; `None` if the host could not make it (logged once).
pub(crate) fn scratch() -> Option<&'static Scratch> {
    static SCRATCH: OnceLock<Option<Scratch>> = OnceLock::new();
    SCRATCH
        .get_or_init(|| {
            let mb = std::env::var("GAMBIARRA_MESH_SCRATCH_MB")
                .ok()
                .and_then(|v| v.trim().parse::<u64>().ok())
                .filter(|&mb| mb > 0)
                .unwrap_or(64);
            let bytes = mb << 20;
            let host = crate::host::host()?;
            let half = bytes / 2;
            let heap = D3D12_HEAP_PROPERTIES {
                Type: D3D12_HEAP_TYPE_DEFAULT,
                ..Default::default()
            };
            let desc = D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                Width: bytes,
                Height: 1,
                DepthOrArraySize: 1,
                MipLevels: 1,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                Flags: D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
                ..Default::default()
            };
            let mut buffer: Option<ID3D12Resource> = None;
            // SAFETY: plain creation of a buffer.
            let made = unsafe {
                host.device.CreateCommittedResource(
                    &heap,
                    D3D12_HEAP_FLAG_NONE,
                    &desc,
                    D3D12_RESOURCE_STATE_COMMON,
                    None,
                    &mut buffer,
                )
            };
            let buffer = match made.ok().and(buffer) {
                Some(b) => b,
                None => {
                    tracing::warn!(mb, "no buffer for emulated mesh dispatches: their draws are dropped");
                    return None;
                }
            };
            // SAFETY: a live buffer.
            let gpu = unsafe { buffer.GetGPUVirtualAddress() };
            tracing::info!(mb, "buffer for emulated mesh dispatches (GAMBIARRA_MESH_SCRATCH_MB)");
            Some(Scratch {
                buffer,
                gpu,
                bytes: half,
                next: AtomicU64::new(0),
            })
        })
        .as_ref()
}

// Indirect mesh dispatches: their group count is in the card's memory. A pass of ours
// (`CONVERT`) reads it, takes a piece of the indirect half of the buffer for that many
// groups with a cursor kept on the card, and writes the commands of the two halves into
// the control buffer: the mesh program as compute (args, output, dispatch) and the draw
// (args, output, vertex count). Both then run as indirect executions.

/// The control buffer: the cursor at byte 0, the compute command at [`COMPUTE_AT`], the
/// draw command at [`DRAW_AT`].
pub(crate) const COMPUTE_AT: u64 = 64;
pub(crate) const DRAW_AT: u64 = 128;
/// The commands' strides: args (4 dwords), a root view's address, the dispatch's three
/// group counts or the draw's four words.
pub(crate) const COMPUTE_STRIDE: u32 = 36;
pub(crate) const DRAW_STRIDE: u32 = 40;

const CONVERT: &str = r#"
#define RS "RootConstants(num32BitConstants=8, b0), SRV(t0), UAV(u0)"
cbuffer Mesh : register(b0) {
    uint args_at;        // the group count's byte in the records
    uint group_bytes;    // a group's vertices and primitives
    uint vertex_dwords;  // a group's vertices, in dwords
    uint prim_vertices;  // vertices drawn per group: three a primitive slot
    uint out_lo;         // the indirect half's address
    uint out_hi;
    uint out_bytes;      // and size
    uint most;           // the groups it holds at once
};
ByteAddressBuffer records : register(t0);
RWByteAddressBuffer control : register(u0);
[RootSignature(RS)]
[numthreads(1, 1, 1)]
void main() {
    uint n = min(records.Load(args_at), most);
    uint bytes = n * group_bytes;
    uint at = (control.Load(0) + 255u) & ~255u;
    if (at + bytes > out_bytes) {
        at = 0;
    }
    control.Store(0, at + bytes);
    uint lo = out_lo + at;
    uint hi = out_hi + (lo < out_lo ? 1u : 0u);
    uint prims = n * vertex_dwords;
    control.Store4(64, uint4(0, prims, 0, 0));
    control.Store2(80, uint2(lo, hi));
    control.Store3(88, uint3(n, 1, 1));
    control.Store4(128, uint4(0, prims, 0, 0));
    control.Store2(144, uint2(lo, hi));
    control.Store4(152, uint4(n * prim_vertices, 1, 0, 0));
}
"#;

pub(crate) struct Convert {
    pub(crate) root: ID3D12RootSignature,
    pub(crate) pso: ID3D12PipelineState,
    /// See [`COMPUTE_AT`].
    pub(crate) control: ID3D12Resource,
}

// SAFETY: D3D12 objects may be used from any thread.
unsafe impl Send for Convert {}
// SAFETY: as above.
unsafe impl Sync for Convert {}

/// The pass and the control buffer, made on first use; `None` if the host refused
/// either (logged once).
pub(crate) fn convert() -> Option<&'static Convert> {
    use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
    use windows::Win32::Graphics::Direct3D::ID3DBlob;
    use windows::core::s;
    static CONVERT_ONCE: OnceLock<Option<Convert>> = OnceLock::new();
    CONVERT_ONCE
        .get_or_init(|| {
            let host = crate::host::host()?;
            let mut code: Option<ID3DBlob> = None;
            let mut errors: Option<ID3DBlob> = None;
            // SAFETY: the source and names live through the call; out pointers ours.
            let compiled = unsafe {
                D3DCompile(
                    CONVERT.as_ptr().cast(),
                    CONVERT.len(),
                    s!("mesh"),
                    None,
                    None,
                    s!("main"),
                    s!("cs_5_1"),
                    0,
                    0,
                    &mut code,
                    Some(&mut errors),
                )
            };
            if compiled.is_err() {
                if let Some(errors) = errors {
                    // SAFETY: the blob's own bytes.
                    let text = unsafe {
                        std::slice::from_raw_parts(
                            errors.GetBufferPointer().cast::<u8>(),
                            errors.GetBufferSize(),
                        )
                    };
                    tracing::warn!("indirect mesh pass: {}", String::from_utf8_lossy(text));
                }
                return None;
            }
            let code = code?;
            // SAFETY: the blob's own bytes; the root signature is inside the shader.
            let (root, pso) = unsafe {
                let bytes = std::slice::from_raw_parts(
                    code.GetBufferPointer().cast::<u8>(),
                    code.GetBufferSize(),
                );
                let root: ID3D12RootSignature = host.device.CreateRootSignature(0, bytes).ok()?;
                let pso: ID3D12PipelineState = host
                    .device
                    .CreateComputePipelineState(&D3D12_COMPUTE_PIPELINE_STATE_DESC {
                        pRootSignature: std::mem::transmute_copy(&root),
                        CS: D3D12_SHADER_BYTECODE {
                            pShaderBytecode: bytes.as_ptr().cast(),
                            BytecodeLength: bytes.len(),
                        },
                        ..Default::default()
                    })
                    .ok()?;
                (root, pso)
            };
            let heap = D3D12_HEAP_PROPERTIES {
                Type: D3D12_HEAP_TYPE_DEFAULT,
                ..Default::default()
            };
            let desc = D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                Width: 256,
                Height: 1,
                DepthOrArraySize: 1,
                MipLevels: 1,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                Flags: D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
                ..Default::default()
            };
            let mut control: Option<ID3D12Resource> = None;
            // SAFETY: plain creation of a buffer.
            unsafe {
                host.device
                    .CreateCommittedResource(
                        &heap,
                        D3D12_HEAP_FLAG_NONE,
                        &desc,
                        D3D12_RESOURCE_STATE_COMMON,
                        None,
                        &mut control,
                    )
                    .ok()?
            };
            Some(Convert {
                root,
                pso,
                control: control?,
            })
        })
        .as_ref()
}

/// The command signatures of an emulated mesh pipeline's two halves: the compute one
/// (on `compute_root`) sets the args at `compute_args`, the output at `compute_out` (a
/// root UAV) and dispatches; the drawing one (on `draw_root`) sets the args and the
/// output (a root SRV) and draws.
#[allow(clippy::too_many_arguments)]
pub(crate) fn signatures(
    compute_root: &ID3D12RootSignature,
    compute_args: u32,
    compute_out: u32,
    draw_root: &ID3D12RootSignature,
    draw_args: u32,
    draw_out: u32,
) -> Option<(ID3D12CommandSignature, ID3D12CommandSignature)> {
    let host = crate::host::host()?;
    let constants = |param| D3D12_INDIRECT_ARGUMENT_DESC {
        Type: D3D12_INDIRECT_ARGUMENT_TYPE_CONSTANT,
        Anonymous: D3D12_INDIRECT_ARGUMENT_DESC_0 {
            Constant: D3D12_INDIRECT_ARGUMENT_DESC_0_1 {
                RootParameterIndex: param,
                DestOffsetIn32BitValues: 0,
                Num32BitValuesToSet: 4,
            },
        },
    };
    let compute = [
        constants(compute_args),
        D3D12_INDIRECT_ARGUMENT_DESC {
            Type: D3D12_INDIRECT_ARGUMENT_TYPE_UNORDERED_ACCESS_VIEW,
            Anonymous: D3D12_INDIRECT_ARGUMENT_DESC_0 {
                UnorderedAccessView: D3D12_INDIRECT_ARGUMENT_DESC_0_4 {
                    RootParameterIndex: compute_out,
                },
            },
        },
        D3D12_INDIRECT_ARGUMENT_DESC {
            Type: D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH,
            ..Default::default()
        },
    ];
    let draw = [
        constants(draw_args),
        D3D12_INDIRECT_ARGUMENT_DESC {
            Type: D3D12_INDIRECT_ARGUMENT_TYPE_SHADER_RESOURCE_VIEW,
            Anonymous: D3D12_INDIRECT_ARGUMENT_DESC_0 {
                ShaderResourceView: D3D12_INDIRECT_ARGUMENT_DESC_0_3 {
                    RootParameterIndex: draw_out,
                },
            },
        },
        D3D12_INDIRECT_ARGUMENT_DESC {
            Type: D3D12_INDIRECT_ARGUMENT_TYPE_DRAW,
            ..Default::default()
        },
    ];
    let make = |args: &[D3D12_INDIRECT_ARGUMENT_DESC], stride, root: &ID3D12RootSignature| {
        let desc = D3D12_COMMAND_SIGNATURE_DESC {
            ByteStride: stride,
            NumArgumentDescs: args.len() as u32,
            pArgumentDescs: args.as_ptr(),
            NodeMask: 0,
        };
        let mut signature: Option<ID3D12CommandSignature> = None;
        // SAFETY: a description of live arguments on a live root signature.
        let made = unsafe { host.device.CreateCommandSignature(&desc, root, &mut signature) };
        if let Err(e) = made {
            tracing::warn!(error = %e, stride, "command signature for indirect mesh dispatches refused");
            return None;
        }
        signature
    };
    Some((
        make(&compute, COMPUTE_STRIDE, compute_root)?,
        make(&draw, DRAW_STRIDE, draw_root)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pieces follow each other on 256 bytes and start over at 0 when the next would
    /// pass the end.
    #[test]
    fn pieces_go_round_the_buffer() {
        let mut next = 0;
        let mut take = |bytes| {
            let at = place(next, bytes, 1024);
            next = at + bytes;
            at
        };
        assert_eq!(take(100), 0);
        assert_eq!(take(300), 256);
        assert_eq!(take(400), 0);
        assert_eq!(take(512), 512);
        assert_eq!(take(1024), 0);
    }
}
