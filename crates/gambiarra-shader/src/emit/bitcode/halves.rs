//! Two console thread groups of 32 lanes in one host wave of 64 (`GAMBIARRA_MERGE_WAVE32`,
//! [`crate::emit::merge_wave32`]): a pass over a finished compute program that makes each
//! half of the host wave (lanes 0..31 and 32..63) behave as the console wave it stands for.
//!
//! A host whose waves are only 64 lanes wide (AMD before RDNA) runs a console group of 32
//! threads in a wave with half its lanes idle. With the group doubled (`[numthreads(64, 1,
//! 1)]`) the host's group `g` holds the console's groups `2g` (lower half) and `2g + 1`
//! (upper half). The program is the console's code once per half, so what reads the group
//! or the thread reads the half's, and what reads other lanes reads only its half's:
//!
//! - group id x: the console group (`group_x`, made by the prologue); thread ids: from the
//!   host lane's place in its half;
//! - lane index: the lane within the half; lane count: 32;
//! - ballots: the half's 32 bits in the first word, nothing above; lane reads: of the same
//!   half; the first lane: the half's first active lane;
//! - reductions, prefixes, any / all, bit counts: over the half only (the other half's
//!   inputs as the operation's identity, or the half's ballot word);
//! - group-shared memory: twice as large, each half at its own offset;
//! - views picked from the heap by a computed index: non-uniform where the index may differ
//!   between the halves (a value one console group has alike may not be the other's); an
//!   index made of constants and constant-buffer reads alone stays uniform.
//!
//! Quad operations stay as they are: a quad never crosses a half. Any other wave operation
//! stops the pass (the program is then translated without merging).

use std::collections::HashSet;

use super::{Attr, Bin, Inst, Module, Op, Ty, TypeDef, V, pred};

/// What the prologue the lowering wrote gives the pass, and where its code begins.
pub(crate) struct Halves {
    /// The host lane (0..63): `waveGetLaneIndex`, made before `from`.
    pub lane: V,
    /// Which half the lane is in: 0 or 1 (i32).
    pub half: V,
    /// The same as a bit.
    pub upper: V,
    /// The console group along x this half stands for.
    pub group_x: V,
    /// The console group's threads.
    pub threads: [u32; 3],
    /// The instructions from this index on are the program's (before it: the prologue).
    pub from: u32,
}

const BARRIER: i64 = 80;
const THREAD_ID: i64 = 93;
const GROUP_ID: i64 = 94;
const THREAD_ID_IN_GROUP: i64 = 95;
const FLAT_THREAD_ID: i64 = 96;
const IS_FIRST_LANE: i64 = 110;
const LANE_INDEX: i64 = 111;
const LANE_COUNT: i64 = 112;
const ANY_TRUE: i64 = 113;
const ALL_TRUE: i64 = 114;
const BALLOT: i64 = 116;
const READ_LANE_AT: i64 = 117;
const READ_LANE_FIRST: i64 = 118;
const ACTIVE_OP: i64 = 119;
const ACTIVE_BIT: i64 = 120;
const PREFIX_OP: i64 = 121;
const QUAD_READ_LANE_AT: i64 = 122;
const QUAD_OP: i64 = 123;
const ALL_BIT_COUNT: i64 = 135;
const PREFIX_BIT_COUNT: i64 = 136;
const HANDLE_FROM_BINDING: i64 = 217;
const HANDLE_FROM_HEAP: i64 = 218;
/// `GAMBIARRA_MERGE_WAVE32_NONUNIFORM=1`: every view picked by a computed index is marked
/// non-uniform, not only those whose index may differ between the halves (an experiment:
/// the driver loops over the distinct indices of such a pick).
pub(crate) fn nonuniform_all() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var_os("GAMBIARRA_MERGE_WAVE32_NONUNIFORM").is_some_and(|v| v == "1")
    })
}

const COUNT_BITS: u32 = 31;
const FIRST_BIT_LO: u32 = 32;

impl Module {
    /// Rewrites the program for two console groups per host wave ([`Halves`]). The
    /// thread-group size and the shared memory's size the caller declares itself; this
    /// doubles the shared memory array. Err: what stopped it (nothing changed is promised
    /// then: the module is to be thrown away).
    pub(crate) fn split_halves(&mut self, h: &Halves) -> Result<(), String> {
        let end = self.insts.len() as u32;
        let range = h.from..end;
        let opcode_of = |m: &Module, i: u32| -> Option<(i64, String)> {
            let Op::Call(f, args) = &m.insts[i as usize].op else {
                return None;
            };
            let name = m.funcs[*f as usize].name.strip_prefix("dx.op.")?;
            let code = m.int_value(*args.first()?)?;
            Some((code, name.to_owned()))
        };
        // The lowering's ballots: what reads them must be an extract of a word.
        let mut ballots = HashSet::new();
        for i in range.clone() {
            if let Some((BALLOT, _)) = opcode_of(self, i) {
                ballots.insert(i);
            }
        }
        for (n, inst) in self.insts.iter().enumerate() {
            if matches!(inst.op, Op::Extract(..)) {
                continue;
            }
            let mut bad = false;
            super::for_each_operand(&inst.op, |v| {
                bad |= matches!(v, V::Inst(b) if ballots.contains(&b));
            });
            if bad {
                return Err(format!("a ballot read whole (instruction {n})"));
            }
        }
        // Group-shared memory: each half at its own offset of an array twice as large.
        let lds = (0..self.globals.len())
            .find(|&g| self.globals[g].space == 3 && self.globals[g].init.is_none());
        let lds = match lds {
            Some(g) => {
                let TypeDef::Array(n, elem) = self.types[self.globals[g].ty.0 as usize].clone()
                else {
                    return Err("group-shared memory that is not an array".into());
                };
                if 2 * u64::from(n) * 4 > 32768 {
                    return Err(format!("{n} dwords of group-shared memory, twice too many"));
                }
                let arr = self.array(2 * n, elem);
                let ptr = self.ptr_in(arr, 3);
                self.globals[g].ty = arr;
                self.globals[g].ptr = ptr;
                Some((V::Global(g as u32), n, arr))
            }
            None => None,
        };
        if let Some((g, ..)) = lds {
            for inst in &self.insts {
                let mut other = false;
                match &inst.op {
                    // (as the address's base: fine)
                    Op::Gep(_, _, idx) => other |= idx.contains(&g),
                    op => super::for_each_operand(op, |v| other |= v == g),
                }
                if other {
                    return Err("group-shared memory used other than by an element address".into());
                }
            }
        }
        let uniform = if nonuniform_all() {
            vec![false; end as usize]
        } else {
            self.dispatch_uniform()
        };
        let k = |m: &mut Module, v: u32| m.i32(v);
        for b in 0..self.blocks.len() {
            let body = std::mem::take(&mut self.blocks[b].body);
            let mut out = Vec::with_capacity(body.len());
            for &i in &body {
                if i < h.from || i >= end {
                    out.push(i);
                    continue;
                }
                // Group-shared memory addresses (any instruction of the program's).
                if let Some((g, n, arr)) = lds
                    && let Op::Gep(_, p, idx) = &self.insts[i as usize].op
                    && *p == g
                {
                    let idx = idx.clone();
                    let [zero, at] = idx[..] else {
                        return Err("a group-shared address of other than one index".into());
                    };
                    let i32t = self.int(32);
                    let n = k(self, n);
                    let base = self.fresh(&mut out, i32t, Op::Bin(Bin::Mul, h.half, n));
                    let at = self.fresh(&mut out, i32t, Op::Bin(Bin::Add, at, base));
                    self.insts[i as usize].op = Op::Gep(arr, g, vec![zero, at]);
                    out.push(i);
                    continue;
                }
                if matches!(&self.insts[i as usize].op, Op::Extract(V::Inst(src), _) if ballots.contains(src))
                {
                    let Op::Extract(src, word) = self.insts[i as usize].op.clone() else {
                        unreachable!()
                    };
                    let i32t = self.int(32);
                    let lo = self.fresh(&mut out, i32t, Op::Extract(src, 0));
                    if word == 0 {
                        let hi = self.fresh(&mut out, i32t, Op::Extract(src, 1));
                        self.insts[i as usize].op = Op::Select(h.upper, hi, lo);
                    } else {
                        // (words above the first: no lane of a 32-lane wave)
                        let z = k(self, 0);
                        self.insts[i as usize].op = Op::Bin(Bin::And, lo, z);
                    }
                    out.push(i);
                    continue;
                }
                let Some((code, name)) = opcode_of(self, i) else {
                    out.push(i);
                    continue;
                };
                let Op::Call(f, args) = self.insts[i as usize].op.clone() else {
                    unreachable!()
                };
                let ty = self.insts[i as usize].ty;
                let i32t = self.int(32);
                let new = match code {
                    GROUP_ID => match self.int_value(args[1]) {
                        Some(0) => {
                            let z = k(self, 0);
                            Some(Op::Bin(Bin::Or, h.group_x, z))
                        }
                        Some(_) => None,
                        None => return Err("a group id of an unknown component".into()),
                    },
                    THREAD_ID_IN_GROUP | THREAD_ID => {
                        let c = self
                            .int_value(args[1])
                            .ok_or("a thread id of an unknown component")?;
                        let local = self.local_id(&mut out, h, c as u32);
                        if code == THREAD_ID_IN_GROUP {
                            Some(Op::Bin(Bin::Or, local, k(self, 0)))
                        } else {
                            let group = if c == 0 {
                                h.group_x
                            } else {
                                let cv = k(self, c as u32);
                                self.op_call(
                                    &mut out,
                                    "groupId.i32",
                                    Attr::ReadNone,
                                    i32t,
                                    GROUP_ID as u32,
                                    &[cv],
                                )
                            };
                            let width = k(self, h.threads[c as usize]);
                            let first = self.fresh(&mut out, i32t, Op::Bin(Bin::Mul, group, width));
                            Some(Op::Bin(Bin::Add, first, local))
                        }
                    }
                    FLAT_THREAD_ID | LANE_INDEX => {
                        let m = k(self, 31);
                        Some(Op::Bin(Bin::And, h.lane, m))
                    }
                    LANE_COUNT => {
                        let n = k(self, 32);
                        Some(Op::Select(h.upper, n, n))
                    }
                    IS_FIRST_LANE => {
                        let lo = self.fresh(&mut out, ty, Op::Call(f, args.clone()));
                        let yes = self.bool(true);
                        let (_, hi_word) = self.ballot(&mut out, yes);
                        let first = self.bits(&mut out, FIRST_BIT_LO, hi_word);
                        let k32 = k(self, 32);
                        let first = self.fresh(&mut out, i32t, Op::Bin(Bin::Add, first, k32));
                        let hi = self.cmp_into(&mut out, pred::EQ, h.lane, first);
                        Some(Op::Select(h.upper, hi, lo))
                    }
                    ANY_TRUE | ALL_TRUE => {
                        let mut bit = args[1];
                        if code == ALL_TRUE {
                            let yes = self.bool(true);
                            let i1 = self.int(1);
                            bit = self.fresh(&mut out, i1, Op::Bin(Bin::Xor, bit, yes));
                        }
                        let word = self.half_word(&mut out, h, bit);
                        let z = k(self, 0);
                        let p = if code == ANY_TRUE { pred::NE } else { pred::EQ };
                        Some(Op::Cmp(p, word, z))
                    }
                    ALL_BIT_COUNT => {
                        let word = self.half_word(&mut out, h, args[1]);
                        let (fi, c) = self.bits_fn(COUNT_BITS);
                        Some(Op::Call(fi, vec![c, word]))
                    }
                    PREFIX_BIT_COUNT => {
                        let all = self.fresh(&mut out, ty, Op::Call(f, args.clone()));
                        let (lo_word, _) = self.ballot(&mut out, args[1]);
                        let below = self.bits(&mut out, COUNT_BITS, lo_word);
                        let z = k(self, 0);
                        let sub = self.fresh(&mut out, i32t, Op::Select(h.upper, below, z));
                        Some(Op::Bin(Bin::Sub, all, sub))
                    }
                    BALLOT => None,
                    READ_LANE_AT => {
                        let m = k(self, 31);
                        let at = self.fresh(&mut out, i32t, Op::Bin(Bin::And, args[2], m));
                        let five = k(self, 5);
                        let base = self.fresh(&mut out, i32t, Op::Bin(Bin::Shl, h.half, five));
                        let at = self.fresh(&mut out, i32t, Op::Bin(Bin::Or, at, base));
                        let mut args = args.clone();
                        args[2] = at;
                        Some(Op::Call(f, args))
                    }
                    READ_LANE_FIRST => {
                        let lo = self.fresh(&mut out, ty, Op::Call(f, args.clone()));
                        let yes = self.bool(true);
                        let (_, hi_word) = self.ballot(&mut out, yes);
                        let first = self.bits(&mut out, FIRST_BIT_LO, hi_word);
                        let k32 = k(self, 32);
                        let first = self.fresh(&mut out, i32t, Op::Bin(Bin::Add, first, k32));
                        let overload = name.rsplit('.').next().unwrap_or("i32").to_owned();
                        let hi = self.op_call(
                            &mut out,
                            &format!("waveReadLaneAt.{overload}"),
                            Attr::NoUnwind,
                            ty,
                            READ_LANE_AT as u32,
                            &[args[1], first],
                        );
                        Some(Op::Select(h.upper, hi, lo))
                    }
                    ACTIVE_OP | ACTIVE_BIT | PREFIX_OP => {
                        let kind = self
                            .int_value(args[2])
                            .ok_or("a wave operation of an unknown kind")?;
                        let signed = match code {
                            ACTIVE_BIT => 0,
                            _ => self
                                .int_value(args[3])
                                .ok_or("a wave operation of unknown sign")?,
                        };
                        let ident = self.identity(code, ty, kind, signed)?;
                        let v = args[1];
                        let hi_in = self.fresh(&mut out, ty, Op::Select(h.upper, v, ident));
                        let mut hi_args = args.clone();
                        hi_args[1] = hi_in;
                        let hi = self.fresh(&mut out, ty, Op::Call(f, hi_args));
                        let lo = if code == PREFIX_OP {
                            // the lanes below a lower lane are all lower ones
                            self.fresh(&mut out, ty, Op::Call(f, args.clone()))
                        } else {
                            let lo_in = self.fresh(&mut out, ty, Op::Select(h.upper, ident, v));
                            let mut lo_args = args.clone();
                            lo_args[1] = lo_in;
                            self.fresh(&mut out, ty, Op::Call(f, lo_args))
                        };
                        Some(Op::Select(h.upper, hi, lo))
                    }
                    QUAD_READ_LANE_AT | QUAD_OP | BARRIER => None,
                    HANDLE_FROM_HEAP | HANDLE_FROM_BINDING => {
                        // index, then (heap) sampler / (binding) the index again; the last
                        // argument is the non-uniform flag
                        let index = if code == HANDLE_FROM_HEAP {
                            args[1]
                        } else {
                            args[2]
                        };
                        // (an index every lane of the dispatch has alike stays uniform)
                        let same = match index {
                            V::Const(_) => true,
                            V::Inst(n) => uniform.get(n as usize).copied().unwrap_or(false),
                            _ => false,
                        };
                        if same {
                            None
                        } else {
                            let mut args = args.clone();
                            let last = args.len() - 1;
                            args[last] = self.bool(true);
                            Some(Op::Call(f, args))
                        }
                    }
                    _ if name.starts_with("wave") || name.starts_with("quad") => {
                        return Err(format!("{name} (dx.op {code})"));
                    }
                    _ => None,
                };
                if let Some(op) = new {
                    self.insts[i as usize].op = op;
                }
                out.push(i);
            }
            self.blocks[b].body = out;
        }
        Ok(())
    }

    /// Per instruction: whether its value is the same in every lane of the whole dispatch
    /// (constants, user data and constant buffers read at such places, and pure arithmetic
    /// of those). Not phis (a join after a branch the halves take apart), nor anything
    /// read from other memory, an id, or a wave operation. A fixed point from "no".
    fn dispatch_uniform(&self) -> Vec<bool> {
        let n = self.insts.len();
        let mut uniform = vec![false; n];
        let pure_call = |f: u32| {
            let name = self.funcs[f as usize].name.as_str();
            let name = name.strip_prefix("dx.op.").unwrap_or(name);
            [
                "cbufferLoadLegacy",
                "createHandleFromBinding",
                "createHandleFromHeap",
                "annotateHandle",
                "binary.",
                "unary.",
                "unaryBits.",
                "tertiary.",
            ]
            .iter()
            .any(|p| name.starts_with(p))
        };
        loop {
            let mut changed = false;
            for i in 0..n {
                if uniform[i] {
                    continue;
                }
                let op = &self.insts[i].op;
                let kind_ok = match op {
                    Op::Bin(..) | Op::Cast(..) | Op::Cmp(..) | Op::Select(..) | Op::Extract(..) => {
                        true
                    }
                    Op::Call(f, _) => pure_call(*f),
                    _ => false,
                };
                if !kind_ok {
                    continue;
                }
                let mut all = true;
                super::for_each_operand(op, |v| {
                    all &= match v {
                        V::Const(_) | V::Func(_) => true,
                        V::Inst(j) => uniform[j as usize],
                        V::Global(_) => false,
                    };
                });
                if all {
                    uniform[i] = true;
                    changed = true;
                }
            }
            if !changed {
                return uniform;
            }
        }
    }

    /// An instruction placed in `out` (not yet in a block).
    fn fresh(&mut self, out: &mut Vec<u32>, ty: Ty, op: Op) -> V {
        let i = self.insts.len() as u32;
        self.insts.push(Inst { ty, op });
        out.push(i);
        V::Inst(i)
    }

    fn cmp_into(&mut self, out: &mut Vec<u32>, p: u8, a: V, b: V) -> V {
        let i1 = self.int(1);
        self.fresh(out, i1, Op::Cmp(p, a, b))
    }

    /// `dx.op.<name>` called with `opcode` and `args`, placed in `out`.
    fn op_call(
        &mut self,
        out: &mut Vec<u32>,
        name: &str,
        attr: Attr,
        ret: Ty,
        opcode: u32,
        args: &[V],
    ) -> V {
        let i32t = self.int(32);
        let mut params = vec![i32t];
        params.extend(args.iter().map(|&a| self.type_of(a)));
        let fty = self.func_ty(ret, params);
        self.ptr(fty);
        let V::Func(f) = self.function(&format!("dx.op.{name}"), fty, Some(attr), false) else {
            unreachable!()
        };
        let mut all = vec![self.i32(opcode)];
        all.extend_from_slice(args);
        self.fresh(out, ret, Op::Call(f, all))
    }

    /// `unaryBits.i32` with `opcode` (31 count, 32 lowest set bit): the function and the
    /// opcode constant.
    fn bits_fn(&mut self, opcode: u32) -> (u32, V) {
        let i32t = self.int(32);
        let fty = self.func_ty(i32t, vec![i32t, i32t]);
        self.ptr(fty);
        let V::Func(f) = self.function("dx.op.unaryBits.i32", fty, Some(Attr::ReadNone), false)
        else {
            unreachable!()
        };
        (f, self.i32(opcode))
    }

    fn bits(&mut self, out: &mut Vec<u32>, opcode: u32, x: V) -> V {
        let (f, c) = self.bits_fn(opcode);
        let i32t = self.int(32);
        self.fresh(out, i32t, Op::Call(f, vec![c, x]))
    }

    /// The host wave's ballot of `bit`: its two words (lanes 0..31, 32..63).
    fn ballot(&mut self, out: &mut Vec<u32>, bit: V) -> (V, V) {
        let i32t = self.int(32);
        let four = self.named_struct("dx.types.fouri32", vec![i32t; 4]);
        let r = self.op_call(
            out,
            "waveActiveBallot",
            Attr::NoUnwind,
            four,
            BALLOT as u32,
            &[bit],
        );
        let lo = self.fresh(out, i32t, Op::Extract(r, 0));
        let hi = self.fresh(out, i32t, Op::Extract(r, 1));
        (lo, hi)
    }

    /// The ballot word of `bit` of this lane's half.
    fn half_word(&mut self, out: &mut Vec<u32>, h: &Halves, bit: V) -> V {
        let (lo, hi) = self.ballot(out, bit);
        let i32t = self.int(32);
        self.fresh(out, i32t, Op::Select(h.upper, hi, lo))
    }

    /// Component `c` of the console thread id of this lane (its place in its half).
    fn local_id(&mut self, out: &mut Vec<u32>, h: &Halves, c: u32) -> V {
        let i32t = self.int(32);
        let [x, y, _] = h.threads;
        let m = self.i32(31);
        let t = self.fresh(out, i32t, Op::Bin(Bin::And, h.lane, m));
        let (xk, yk, xy) = (self.i32(x), self.i32(y), self.i32(x * y));
        match c {
            0 => self.fresh(out, i32t, Op::Bin(Bin::URem, t, xk)),
            1 => {
                let row = self.fresh(out, i32t, Op::Bin(Bin::UDiv, t, xk));
                self.fresh(out, i32t, Op::Bin(Bin::URem, row, yk))
            }
            _ => self.fresh(out, i32t, Op::Bin(Bin::UDiv, t, xy)),
        }
    }

    /// The value that leaves the other operand of wave operation `code` (`kind`, `signed`:
    /// its arguments) unchanged, of type `ty`.
    fn identity(&mut self, code: i64, ty: Ty, kind: i64, signed: i64) -> Result<V, String> {
        let def = self.types[ty.0 as usize].clone();
        let bits: u32 = match (code, &def, kind) {
            // WaveActiveBit: And, Or, Xor
            (ACTIVE_BIT, TypeDef::Int(32), 0) => u32::MAX,
            (ACTIVE_BIT, TypeDef::Int(32), 1 | 2) => 0,
            // WaveActiveOp / WavePrefixOp: Sum, Product, Min, Max (prefixes: the first two)
            (_, TypeDef::Int(32), 0) => 0,
            (_, TypeDef::Int(32), 1) => 1,
            (ACTIVE_OP, TypeDef::Int(32), 2) if signed == 0 => 0x7FFF_FFFF,
            (ACTIVE_OP, TypeDef::Int(32), 2) => u32::MAX,
            (ACTIVE_OP, TypeDef::Int(32), 3) if signed == 0 => 0x8000_0000,
            (ACTIVE_OP, TypeDef::Int(32), 3) => 0,
            // -0.0 leaves every sum unchanged, +0.0 not -0.0
            (_, TypeDef::Float, 0) => 0x8000_0000,
            (_, TypeDef::Float, 1) => 0x3F80_0000,
            (ACTIVE_OP, TypeDef::Float, 2) => 0x7F80_0000,
            (ACTIVE_OP, TypeDef::Float, 3) => 0xFF80_0000,
            _ => {
                return Err(format!("a wave operation {code} of kind {kind} on {def:?}"));
            }
        };
        Ok(match def {
            TypeDef::Float => self.f32(bits),
            _ => self.i32(bits),
        })
    }
}
