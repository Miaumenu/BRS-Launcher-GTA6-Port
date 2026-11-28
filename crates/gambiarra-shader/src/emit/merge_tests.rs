//! Two console groups of 32 threads in each host group of 64 (`GAMBIARRA_MERGE_WAVE32`,
//! `bitcode::halves`): a synthetic wave32 program that reads its group id, a lane read, a
//! mask read as a number and group-shared memory, lowered with and without the merge, run
//! on the host GPU when its waves are 64 lanes, and the translations of a folder of the
//! title's compute containers (`GAMBIARRA_MERGE_CORPUS`) checked by the validator.

use super::host::{Dispatch, Host, Root, View};
use super::lower::{TEST_HOST_MIN_WAVE, TEST_HOST_WAVE};
use super::{HostShader, TEST_MERGE_WAVE32, USER_MERGED_GROUPS_X, lower, validator};
use crate::native::{
    ComputeIo, HwStage, NativeProgram, ResourceType, RootParam, RootType, StageIo, UsedResource,
};
use crate::{io, ir, resource, structure};

const S_ENDPGM: u32 = 0xBF81_0000;
const S_BARRIER: u32 = 0xBF8A_0000;
/// `s_waitcnt lgkmcnt(0)`.
const S_WAIT_LGKM: u32 = 0xBF8C_C07F;
const VCC_LO: u8 = 106;

const fn s_mov_b32(sdst: u8, src0: u8) -> u32 {
    0xBE80_0000 | (sdst as u32) << 16 | 0x03 << 8 | src0 as u32
}
/// SOP1 `op sdst, src0`.
const fn sop1(op: u8, sdst: u8, src0: u8) -> u32 {
    0xBE80_0000 | (sdst as u32) << 16 | (op as u32) << 8 | src0 as u32
}
const fn s_load_dwordx4(dst: u8, base: u8) -> [u32; 2] {
    [
        0x3D << 26 | 0x02 << 18 | (dst as u32) << 6 | (base as u32 >> 1),
        125 << 25,
    ]
}
/// VOP1 `op vdst, src0` (`src0`: 0..105 SGPR, 128.. inline constants, 256.. VGPR).
const fn vop1(op: u16, vdst: u8, src0: u16) -> u32 {
    0x7E00_0000 | (vdst as u32) << 17 | (op as u32) << 9 | (src0 as u32 & 0x1FF)
}
/// VOP2 `op vdst, src0, vsrc1`.
const fn vop2(op: u8, vdst: u8, src0: u16, vsrc1: u8) -> u32 {
    (op as u32 & 0x3F) << 25 | (vdst as u32) << 17 | (vsrc1 as u32) << 9 | (src0 as u32 & 0x1FF)
}
/// VOPC `op vcc, src0, vsrc1`.
const fn vopc(op: u8, src0: u16, vsrc1: u8) -> u32 {
    0x3E << 25 | (op as u32) << 17 | (vsrc1 as u32) << 9 | (src0 as u32 & 0x1FF)
}
/// DS `op` with `addr` and `data0` (a store) or `vdst` (a load).
const fn ds(op: u8, vdst: u8, addr: u8, data0: u8) -> [u32; 2] {
    [
        0x36 << 26 | (op as u32) << 18,
        (vdst as u32) << 24 | (data0 as u32) << 8 | addr as u32,
    ]
}
/// MUBUF `buffer_store_dword vdata, vaddr, s[srsrc:srsrc+3], 0 offen offset:byte_off`.
const fn store_offen(vdata: u8, vaddr: u8, srsrc: u8, byte_off: u16) -> [u32; 2] {
    [
        0x38 << 26 | 0x1C << 18 | 1 << 12 | (byte_off as u32 & 0xFFF),
        0x80 << 24 | ((srsrc as u32 >> 2) & 0x1F) << 16 | (vdata as u32) << 8 | vaddr as u32,
    ]
}
const fn vgpr(n: u8) -> u16 {
    256 + n as u16
}
const fn inline(n: u8) -> u16 {
    128 + n as u16
}

/// Groups of 32 threads in x; group id x in s5 (after the five user SGPRs: four root
/// constants, then the table holding the UAV). Each thread stores four dwords at
/// `(group * 32 + thread) * 16`: `group << 10 | thread`, the first lane's value of that, the
/// number of lanes whose thread is above the group number (a mask read as a number), and the
/// value its neighbour (`thread ^ 1`) wrote to group-shared memory.
fn program() -> Vec<u32> {
    let mut w = vec![
        vop1(0x01, 5, 5), // v_mov_b32 v5, s5 (the group)
        s_mov_b32(5, 0x80),
    ];
    w.extend_from_slice(&s_load_dwordx4(8, 4));
    w.extend_from_slice(&[
        S_WAIT_LGKM,
        vop2(0x1A, 6, inline(10), 5), // v_lshlrev_b32 v6, 10, v5
        vop2(0x25, 6, vgpr(0), 6),    // v_add_nc_u32 v6, v0, v6
        vop1(0x02, 20, vgpr(6)),      // v_readfirstlane_b32 s20, v6
        vop1(0x01, 7, 20),            // v_mov_b32 v7, s20
        vopc(0xC1, vgpr(5), 0),       // v_cmp_lt_u32 vcc, v5, v0
        sop1(0x0F, 21, VCC_LO),       // s_bcnt1_i32_b32 s21, vcc_lo
        vop1(0x01, 9, 21),            // v_mov_b32 v9, s21
        vop2(0x1A, 10, inline(2), 0), // v_lshlrev_b32 v10, 2, v0
    ]);
    w.extend_from_slice(&ds(0x0D, 0, 10, 6)); // ds_write_b32 v10, v6
    w.extend_from_slice(&[S_WAIT_LGKM, S_BARRIER]);
    w.push(vop2(0x1D, 11, inline(4), 10)); // v_xor_b32 v11, 4, v10
    w.extend_from_slice(&ds(0x36, 8, 11, 0)); // ds_read_b32 v8, v11
    w.extend_from_slice(&[
        S_WAIT_LGKM,
        vop2(0x1A, 12, inline(5), 5),  // v_lshlrev_b32 v12, 5, v5
        vop2(0x25, 12, vgpr(0), 12),   // v_add_nc_u32 v12, v0, v12
        vop2(0x1A, 12, inline(4), 12), // v_lshlrev_b32 v12, 4, v12
    ]);
    for (n, v) in [6u8, 7, 9, 8].into_iter().enumerate() {
        w.extend_from_slice(&store_offen(v, 12, 8, 4 * n as u16));
    }
    w.push(S_ENDPGM);
    w
}

/// What thread `t` of group `g` stores.
fn expected(g: u32, t: u32) -> [u32; 4] {
    [
        g << 10 | t,
        g << 10,
        (g + 1..32).count() as u32,
        g << 10 | (t ^ 1),
    ]
}

/// The program lowered for a host of 64-lane waves, merged or not.
fn build(merge: bool) -> HostShader {
    let code: Vec<u8> = program().iter().flat_map(|w| w.to_le_bytes()).collect();
    let native = NativeProgram {
        stage: HwStage::Cs,
        wave64: false,
        vgprs: 16,
        layout: vec![
            RootParam {
                root: RootType::Constants,
                table_offset_dwords: 0,
                sgprs: 4,
                base_register: 0,
                user_data: 0,
                slot: 0,
            },
            RootParam {
                root: RootType::Table,
                table_offset_dwords: 0,
                sgprs: 1,
                base_register: 0,
                user_data: 4,
                slot: 1,
            },
        ],
        used: vec![UsedResource {
            kind: ResourceType::Uav,
            root: RootType::Table,
            table_offset_dwords: 0,
            lower_bound: 0,
            stride: 0,
            dwords: 4,
            user_data: 4,
            slot: 1,
        }],
        io: StageIo::Cs(ComputeIo {
            // 5 user SGPRs, the group id in x, 512 bytes of LDS
            rsrc2: (5 << 1) | (1 << 7) | (1 << 15),
            threads: [32, 1, 1],
        }),
        code,
    };
    TEST_HOST_WAVE.set(Some(64));
    TEST_HOST_MIN_WAVE.set(Some(64));
    TEST_MERGE_WAVE32.set(Some(merge));
    let p = ir::build(&native.code, native.wave64, native.vgprs).expect("ir::build");
    let st = structure::structure(&p).expect("structure");
    let res =
        resource::track(&p, &native.layout, native.stage, Some(&st)).expect("resource::track");
    let io = io::raise(&p, &native).expect("io::raise");
    let lowered = lower::lower(&lower::Parts {
        native: &native,
        p: &p,
        st: &st,
        io: &io,
        res: &res,
        inputs: Vec::new(),
        outputs: Vec::new(),
        prims: Vec::new(),
        mesh_stride: 0,
    });
    let outcome = lower::take_merge_outcome();
    TEST_MERGE_WAVE32.set(None);
    let lowered = lowered.expect("lower");
    if merge {
        assert!(matches!(outcome, Some(Ok(_))), "not merged: {outcome:?}");
    } else {
        assert!(outcome.is_none());
    }
    let shader = HostShader {
        stage: HwStage::Cs,
        dxil: lowered.dxil,
        user_data: lowered.user_data,
        samplers: lowered.samplers,
        static_samplers: Vec::new(),
        root_cbvs: lowered.root_cbvs,
        root_srvs: lowered.root_srvs,
        root_uavs: lowered.root_uavs,
        root_indexed_views: res.root_indexed_views,
    };
    if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
        v.validate(&mut shader.dxil.clone())
            .unwrap_or_else(|e| panic!("merged {merge}: DXIL validation failed: {e}"));
    } else {
        eprintln!("no dxil.dll: the translation is not validated");
    }
    shader
}

#[test]
fn a_merged_program_asks_for_the_titles_group_count() {
    let merged = build(true);
    assert!(
        merged
            .user_data
            .iter()
            .any(|u| u.root == RootType::Other(USER_MERGED_GROUPS_X))
    );
    let plain = build(false);
    assert!(
        !plain
            .user_data
            .iter()
            .any(|u| u.root == RootType::Other(USER_MERGED_GROUPS_X))
    );
}

/// Windows 10's own D3D12 stops at SM 6.5: the Agility SDK the launcher ships
/// (`GAMBIARRA_D3D12_DIR`, else `target\diag\D3D12`), for this process, when there is one.
fn agility() {
    use windows::Win32::Graphics::Direct3D12::*;
    static ONCE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        let dir = std::env::var_os("GAMBIARRA_D3D12_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/diag/D3D12")
            });
        let Ok(dir) = std::path::absolute(&dir) else {
            return;
        };
        let dll = dir.join("D3D12Core.dll");
        if !dll.is_file() {
            return;
        }
        let wide: Vec<u16> = dll.as_os_str().encode_wide().chain([0]).collect();
        // SAFETY: a NUL-terminated path; the module stays loaded, its version export is a u32.
        let version = unsafe {
            let m = windows_sys::Win32::System::LibraryLoader::LoadLibraryW(wide.as_ptr());
            if m.is_null() {
                return;
            }
            match windows_sys::Win32::System::LibraryLoader::GetProcAddress(
                m,
                c"D3D12SDKVersion".as_ptr().cast(),
            ) {
                Some(p) => *(p as *const u32),
                None => return,
            }
        };
        let path = std::ffi::CString::new(format!("{}\\", dir.display())).unwrap();
        // SAFETY: plain D3D12 configuration calls with a NUL-terminated path.
        unsafe {
            let mut cfg: Option<ID3D12SDKConfiguration1> = None;
            if D3D12GetInterface(&CLSID_D3D12SDKConfiguration, &mut cfg).is_err() {
                return;
            }
            let Some(cfg) = cfg else { return };
            if let Ok(factory) = cfg.CreateDeviceFactory::<_, ID3D12DeviceFactory>(
                version,
                windows::core::PCSTR(path.as_ptr().cast()),
            ) {
                let _ = factory.ApplyToGlobalState();
                std::mem::forget(factory);
            }
        }
    });
}

use std::os::windows::ffi::OsStrExt;

/// The host's narrowest and widest wave.
fn wave_lanes(host: &Host) -> (u32, u32) {
    use windows::Win32::Graphics::Direct3D12::*;
    let mut o = D3D12_FEATURE_DATA_D3D12_OPTIONS1::default();
    // SAFETY: a live device and a matching out structure.
    let ok = unsafe {
        host.device.CheckFeatureSupport(
            D3D12_FEATURE_D3D12_OPTIONS1,
            (&raw mut o).cast(),
            size_of_val(&o) as u32,
        )
    };
    if ok.is_err() {
        return (0, 0);
    }
    (o.WaveLaneCountMin, o.WaveLaneCountMax)
}

#[test]
fn merged_groups_compute_what_single_groups_do() {
    const GROUPS: u32 = 5;
    const DWORDS: u32 = 4 * 32 * (GROUPS + 3);
    let plain = build(false);
    let merged = build(true);
    let _one = super::valu_tests::ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    agility();
    let host = match Host::new(false) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("no SM 6.6 hardware device ({e}); skipped");
            return;
        }
    };
    if wave_lanes(&host) != (64, 64) {
        eprintln!(
            "host waves {:?}, not 64 lanes only; skipped",
            wave_lanes(&host)
        );
        return;
    }
    let run = |shader: &HostShader, groups: u32, count: u32| -> Vec<u32> {
        let (root, pso) = host.compute_pipeline(shader).expect("pipeline");
        let mut ud = vec![0u32; shader.user_data_dwords().max(1) as usize];
        for u in &shader.user_data {
            ud[u.index as usize] = match u.root {
                RootType::Other(USER_MERGED_GROUPS_X) => count,
                _ => 0,
            };
        }
        let out = host
            .dispatch(
                &root,
                &pso,
                &Dispatch {
                    roots: vec![(0, Root::Constants(ud))],
                    views: &[View::RawUav(DWORDS)],
                    samplers: &[],
                    groups: [groups, 1, 1],
                },
            )
            .expect("dispatch");
        out[0].clone()
    };
    let check = |what: &str, got: &[u32]| {
        for g in 0..GROUPS + 3 {
            for t in 0..32 {
                let at = (4 * (g * 32 + t)) as usize;
                let want = if g < GROUPS { expected(g, t) } else { [0; 4] };
                assert_eq!(got[at..at + 4], want, "{what}: group {g} thread {t}");
            }
        }
    };
    check("single groups", &run(&plain, GROUPS, 0));
    // two console groups a host group, the last upper half past the count
    check("merged", &run(&merged, GROUPS.div_ceil(2), GROUPS));
    // a host that does not set the count: one console group a host group
    check("merged program, title's dispatch", &run(&merged, GROUPS, 0));
}

/// `GAMBIARRA_MERGE_CORPUS=<folder>`: every `*.compute.cdxil` there (the title's containers,
/// as `GAMBIARRA_SHADER_SAVE` keeps them) translated with the merge for a host of 64-lane
/// waves; the merged ones must pass the validator. Prints what was merged and skipped.
#[test]
fn the_titles_merged_programs_validate() {
    let Some(dir) = std::env::var_os("GAMBIARRA_MERGE_CORPUS") else {
        eprintln!("GAMBIARRA_MERGE_CORPUS unset; skipped");
        return;
    };
    TEST_HOST_WAVE.set(Some(64));
    TEST_HOST_MIN_WAVE.set(Some(64));
    let mut validator =
        validator::Validator::load(validator::Validator::default_path().as_deref()).ok();
    let (mut merged, mut other, mut failed) = (0, 0, Vec::new());
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("corpus folder")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".compute.cdxil"))
        .collect();
    entries.sort();
    for path in entries {
        let bytes = std::fs::read(&path).expect("container");
        TEST_MERGE_WAVE32.set(Some(true));
        let r = super::translate(&bytes);
        TEST_MERGE_WAVE32.set(None);
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let label = format!("{name} {}", super::program_name(&bytes));
        let Ok(shader) = r else { continue };
        let asks = shader
            .user_data
            .iter()
            .any(|u| u.root == RootType::Other(USER_MERGED_GROUPS_X));
        if !asks {
            other += 1;
            continue;
        }
        merged += 1;
        if let Some(v) = validator.as_mut()
            && let Err(e) = v.validate(&mut shader.dxil.clone())
        {
            failed.push(format!("{label}: {e}"));
        }
    }
    eprintln!(
        "merged {merged}, not merged {other}, failed validation {}",
        failed.len()
    );
    for f in &failed {
        eprintln!("  {f}");
    }
    assert!(validator.is_some(), "no dxil.dll to validate with");
    assert!(failed.is_empty());
}
