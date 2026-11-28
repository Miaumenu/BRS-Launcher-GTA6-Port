//! The vertex shader that draws what a mesh program translated as compute stored
//! ([`super::translate_mesh_as_compute`]), on hosts without mesh shaders. The draw is a
//! non-indexed triangle list of three vertices per primitive slot of the dispatch's groups;
//! vertex `3p + k` is corner `k` of primitive `p`. Its outputs are the pixel shader's inputs,
//! register for register, each taken from the corner's vertex or, for per-primitive
//! outputs, from the primitive (constant over the triangle). A culled primitive names one
//! vertex three times: a triangle without area, which the host does not rasterise.

use super::bitcode::{Attr, Bin, Cast, Term, V};
use super::container::SigElement;
use super::shader::{MESH_ARG_PRIMS, SLOT_MESH_ARGS, SLOT_MESH_OUT, Shader};
use super::{HostShader, MeshLayout};
use crate::ShaderError;
use crate::dxbc::{Container, FourCC};
use crate::native::HwStage;

/// The vertex shader for a mesh program of `layout`, linked to the pixel shader `ps` (its
/// DXIL, [`HostShader::dxil`]), or with a position alone when there is none. It reads the
/// output at the root SRV `t<SLOT_MESH_OUT>` and the args at the root CBV
/// `b<SLOT_MESH_ARGS>`, both in [`super::ROOT_CBV_SPACE`]: bind them as for the dispatch.
pub fn mesh_reader_vs(layout: &MeshLayout, ps: Option<&[u8]>) -> Result<HostShader, ShaderError> {
    let wanted = match ps {
        Some(ps) => {
            let c = Container::parse(ps)?;
            match c.part(FourCC::ISG1) {
                Some(part) => super::signature(part.data)?,
                None => Vec::new(),
            }
        }
        None => Vec::new(),
    };
    build(layout, &wanted)
}

/// Where an output's column comes from: the primitive (else the vertex) and the dword in
/// its record; none for a column the mesh program does not write (0).
type Source = Option<(bool, u32)>;

fn build(layout: &MeshLayout, ps_inputs: &[SigElement]) -> Result<HostShader, ShaderError> {
    let unsupported = |what| ShaderError::Unsupported { offset: 0, what };
    let same = |a: &SigElement, b: &SigElement| {
        a.kind == b.kind && a.index == b.index && a.name.eq_ignore_ascii_case(&b.name)
    };
    let find = |e: &SigElement| -> Option<(bool, &SigElement, u32)> {
        let vertex = layout.vertex_outputs.iter().find(|(o, _)| same(o, e));
        let primitive = layout.primitive_outputs.iter().find(|(o, _)| same(o, e));
        vertex
            .map(|(o, at)| (false, o, *at))
            .or(primitive.map(|(o, at)| (true, o, *at)))
    };
    // What the pixel shader reads that a vertex shader writes: arbitrary values, the
    // position, clip and cull distances.
    let mut outputs: Vec<SigElement> = Vec::new();
    let mut sources: Vec<Vec<Source>> = Vec::new();
    for e in ps_inputs
        .iter()
        .filter(|e| matches!(e.kind, 0 | 3 | 6 | 7) && e.row.is_some())
    {
        let found = find(e);
        sources.push(
            (0..e.cols)
                .map(|k| match found {
                    Some((primitive, o, at)) if k < o.cols => {
                        Some((primitive, at + u32::from(k) + if primitive { 3 } else { 0 }))
                    }
                    _ => None,
                })
                .collect(),
        );
        outputs.push(SigElement {
            usage: (1 << e.cols) - 1,
            ..e.clone()
        });
    }
    if !outputs.iter().any(|e| e.kind == 3) {
        let (o, at) = layout
            .vertex_outputs
            .iter()
            .find(|(o, _)| o.kind == 3)
            .ok_or(unsupported("a mesh program without a position"))?;
        let row = outputs
            .iter()
            .filter_map(|e| e.row)
            .map(|r| r + 1)
            .max()
            .unwrap_or(0);
        sources.push((0..o.cols).map(|k| Some((false, at + u32::from(k)))).collect());
        outputs.push(SigElement {
            interp: 4,
            row: Some(row),
            col: 0,
            usage: (1 << o.cols) - 1,
            ..o.clone()
        });
    }

    // Interpolation: the pixel shader's own (elements of a row share theirs there; one
    // made up per kind put a flat float beside a smooth one and the host refused the
    // pipeline), and as `emit_program` declares vertex outputs for a pixel shader that
    // left it out.
    for e in &mut outputs {
        if e.interp == 0 {
            e.interp = match (e.kind, e.comp) {
                (3, _) => 4,
                (0 | 6 | 7, 9) => 2,
                (0, _) => 1,
                _ => 0,
            };
        }
    }
    let mut s = Shader::new(false);
    s.inputs = vec![SigElement {
        name: "SV_VertexID".into(),
        index: 0,
        kind: 1,
        comp: 5,
        interp: 0,
        row: Some(0),
        col: 0,
        cols: 1,
        usage: 1,
        half: false,
    }];
    s.outputs = outputs;
    let b = s.entry;
    let i32t = s.t.i32;
    let iu = s.m.undef(i32t);
    let args = [s.m.i32(0), s.m.i32(0), s.m.i8(0), iu];
    let id = s.op(b, "loadInput.i32", Attr::ReadNone, i32t, 4, &args);
    let k3 = s.m.i32(3);
    let primitive = s.m.bin(b, Bin::UDiv, id, k3);
    let corner = s.m.bin(b, Bin::URem, id, k3);

    let h_args = s.root_cbv_handle(SLOT_MESH_ARGS);
    let row = s.m.i32(MESH_ARG_PRIMS / 4);
    let r = s.op(
        b,
        "cbufferLoadLegacy.i32",
        Attr::ReadOnly,
        s.t.cbuf_ret,
        59,
        &[h_args, row],
    );
    let prims = s.m.extract(b, r, MESH_ARG_PRIMS % 4);
    let h_out = s.root_srv_handle(SLOT_MESH_OUT);
    let pdw = s.m.i32(layout.primitive_dwords);
    let at = s.m.bin(b, Bin::Mul, primitive, pdw);
    let prim_record = s.m.bin(b, Bin::Add, prims, at);
    let slot = s.m.bin(b, Bin::Add, prim_record, corner);
    let vertex = load(&mut s, h_out, slot);
    let vertex = s.m.cast(b, Cast::Bitcast, vertex, i32t);
    let vdw = s.m.i32(layout.vertex_dwords);
    let vertex_record = s.m.bin(b, Bin::Mul, vertex, vdw);

    for (id, columns) in sources.iter().enumerate() {
        let float = s.outputs[id].comp == 9;
        for (col, source) in columns.iter().enumerate() {
            let value = match *source {
                Some((from_primitive, dword)) => {
                    let base = if from_primitive {
                        prim_record
                    } else {
                        vertex_record
                    };
                    let k = s.m.i32(dword);
                    let place = s.m.bin(b, Bin::Add, base, k);
                    load(&mut s, h_out, place)
                }
                None => s.m.f32(0),
            };
            let (name, value) = if float {
                ("storeOutput.f32", value)
            } else {
                ("storeOutput.i32", s.m.cast(b, Cast::Bitcast, value, i32t))
            };
            let args = [s.m.i32(id as u32), s.m.i32(0), s.m.i8(col as u8), value];
            s.op(b, name, Attr::NoUnwind, s.t.void, 5, &args);
        }
    }
    s.m.terminate(b, Term::Ret);
    let root_cbvs = s.root_cbvs.iter().map(|r| r.0).collect();
    let root_srvs = s.root_srvs.iter().map(|r| r.0).collect();
    Ok(HostShader {
        stage: HwStage::Vs,
        dxil: s.finish(),
        user_data: Vec::new(),
        samplers: Vec::new(),
        static_samplers: Vec::new(),
        root_cbvs,
        root_srvs,
        root_uavs: Vec::new(),
        root_indexed_views: Vec::new(),
    })
}

/// Dword `index` of the output, as a float.
fn load(s: &mut Shader, h: V, index: V) -> V {
    let b = s.entry;
    let k2 = s.m.i32(2);
    let at = s.m.bin(b, Bin::Shl, index, k2);
    let iu = s.m.undef(s.t.i32);
    let (mask, align) = (s.m.i8(1), s.m.i32(4));
    let r = s.op(
        b,
        "rawBufferLoad.f32",
        Attr::ReadOnly,
        s.t.res_ret,
        139,
        &[h, at, iu, mask, align],
    );
    s.m.extract(b, r, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(name: &str, index: u32, kind: u8, comp: u8, row: u32, col: u8, cols: u8) -> SigElement {
        SigElement {
            name: name.into(),
            index,
            kind,
            comp,
            interp: 0,
            row: Some(row),
            col,
            cols,
            usage: (1 << cols) - 1,
            half: false,
        }
    }

    /// A position and a float2 per vertex, a uint per primitive.
    fn layout() -> MeshLayout {
        MeshLayout {
            threads: 64,
            max_vertices: 64,
            max_primitives: 126,
            vertex_dwords: 6,
            primitive_dwords: 4,
            vertex_outputs: vec![
                (element("SV_Position", 0, 3, 9, 0, 0, 4), 0),
                (element("TEXCOORD", 0, 0, 9, 1, 0, 2), 4),
            ],
            primitive_outputs: vec![(element("TEXCOORD", 1, 0, 5, 0, 0, 1), 0)],
        }
    }

    /// The reader is a vertex shader of one vertex-id input whose outputs are the pixel
    /// shader's inputs, the position added when the pixel shader does not read it; the
    /// validator, where there is one, accepts it.
    #[test]
    fn the_reader_outputs_what_the_pixel_shader_reads() {
        let with_position = [
            SigElement {
                interp: 4,
                ..element("SV_Position", 0, 3, 9, 0, 0, 4)
            },
            SigElement {
                interp: 2,
                ..element("TEXCOORD", 0, 0, 9, 1, 0, 2)
            },
            SigElement {
                interp: 1,
                ..element("TEXCOORD", 1, 0, 5, 2, 0, 1)
            },
        ];
        let without_position = [with_position[2].clone()];
        let mut failed = Vec::new();
        for (case, ps_inputs) in [&with_position[..], &without_position[..], &[][..]]
            .into_iter()
            .enumerate()
        {
            let vs = build(&layout(), ps_inputs).unwrap();
            assert_eq!(vs.root_cbvs, [SLOT_MESH_ARGS]);
            assert_eq!(vs.root_srvs, [SLOT_MESH_OUT]);
            let c = Container::parse(&vs.dxil).unwrap();
            // Program version: vertex (1), shader model 6.6.
            assert_eq!(c.part(FourCC::DXIL).unwrap().data[..4], [0x66, 0, 1, 0]);
            let outs = super::super::signature(c.part(FourCC::OSG1).unwrap().data).unwrap();
            assert_eq!(outs.len(), ps_inputs.len().max(1) + usize::from(ps_inputs.len() == 1));
            for (o, i) in outs.iter().zip(ps_inputs) {
                assert_eq!((&o.name, o.index, o.row), (&i.name, i.index, i.row));
            }
            assert!(outs.iter().any(|o| o.kind == 3));
            #[cfg(windows)]
            if let Ok(mut v) = super::super::validator::Validator::load(
                super::super::validator::Validator::default_path().as_deref(),
            ) {
                if let Err(e) = v.validate(&mut vs.dxil.clone()) {
                    eprintln!("case {case}: {e}");
                    failed.push(case);
                }
            }
        }
        assert!(failed.is_empty(), "cases {failed:?} refused");
    }

    /// A title's pair, as `GAMBIARRA_SHADER_SAVE` wrote them: `GAMBIARRA_MESH_PAIR` is the
    /// mesh container (`.cdxil`) and the translated pixel shader (`.dxil`), `;` between.
    /// The reader is made and validated, with the mesh program as compute.
    #[test]
    #[ignore]
    fn the_reader_for_a_saved_pair_validates() {
        let pair = std::env::var("GAMBIARRA_MESH_PAIR").expect("GAMBIARRA_MESH_PAIR");
        let (ms, ps) = pair.split_once(';').expect("two paths");
        let (ms, ps) = (std::fs::read(ms).unwrap(), std::fs::read(ps).unwrap());
        let (cs, layout) = super::super::translate_mesh_as_compute(&ms).unwrap();
        eprintln!(
            "layout: threads {} vertices {} primitives {} dwords {}/{}",
            layout.threads,
            layout.max_vertices,
            layout.max_primitives,
            layout.vertex_dwords,
            layout.primitive_dwords
        );
        for (e, at) in layout.vertex_outputs.iter().chain(&layout.primitive_outputs) {
            eprintln!("  {}{} kind {} comp {} row {:?} cols {} at {at}", e.name, e.index, e.kind, e.comp, e.row, e.cols);
        }
        let vs = mesh_reader_vs(&layout, Some(&ps)).unwrap();
        #[cfg(windows)]
        {
            let mut v = super::super::validator::Validator::load(
                super::super::validator::Validator::default_path().as_deref(),
            )
            .unwrap();
            if let Err(e) = v.validate(&mut cs.dxil.clone()) {
                eprintln!("compute: {e}");
            }
            if let Err(e) = v.validate(&mut vs.dxil.clone()) {
                eprintln!("reader: {e}");
            }
            if let Err(e) = v.validate(&mut ps.clone()) {
                eprintln!("pixel: {e}");
            }
        }
    }

    #[test]
    fn a_mesh_program_without_a_position_is_refused() {
        let mut l = layout();
        l.vertex_outputs.remove(0);
        assert!(build(&l, &[]).is_err());
    }
}
