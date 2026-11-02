//! The stock module tree (`re/dsp/graph.py`): every module header in RAM, containers with their children in
//! execution order. Module at `addr`: +0 inputs, +4 outputs, +8 scratch, +12 → wire list (inputs, outputs,
//! scratch), +16 flags (0x3 audio, 0x803 control, 0x7 container), +20 process, +24 init, +28 parameters.
//! Container: +52 child count, +56 → child offsets in words from the container (0 = empty slot).

use super::image::Ram;
use super::ResponseError;

/// Root container of the stock graph.
pub const ROOT: u32 = 0x1001_00C0;

pub const FLAG_CONTAINER: u32 = 0x7;
pub const FLAG_CONTROL: u32 = 0x803;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Biquad,
    Mixer,
    Gain,
    Delay,
    SumDiff,
    Meter,
    Router,
    Adder,
    Other(u32),
}

impl Kind {
    fn of(proc_: u32) -> Self {
        match proc_ {
            0x1002_C904 => Kind::Biquad,
            0x1002_C260 => Kind::Mixer,
            0x1002_C5B0 => Kind::Gain,
            0x1001_AD94 => Kind::Delay,
            0x1002_D668 => Kind::SumDiff,
            0x1002_CDDC => Kind::Meter,
            0x1001_C928 => Kind::Router,
            0x1001_CE10 => Kind::Adder,
            p => Kind::Other(p),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Module {
    pub addr: u32,
    pub nin: usize,
    pub nout: usize,
    pub flags: u32,
    pub kind: Kind,
    pub ins: Vec<u32>,
    pub outs: Vec<u32>,
    pub params: u32,
    pub children: Vec<Module>,
}

const MAX_DEPTH: usize = 8;
const MAX_WIRES: usize = 64;

impl Module {
    pub fn read(ram: &Ram, addr: u32) -> Result<Self, ResponseError> {
        Self::read_at(ram, addr, 0)
    }

    fn read_at(ram: &Ram, addr: u32, depth: usize) -> Result<Self, ResponseError> {
        if depth > MAX_DEPTH {
            return Err(ResponseError::Graph(format!("module tree too deep at 0x{addr:08x}")));
        }
        let nin = ram.u(addr)? as usize;
        let nout = ram.u(addr + 4)? as usize;
        let nscr = ram.u(addr + 8)? as usize;
        let io = ram.u(addr + 12)?;
        let flags = ram.u(addr + 16)?;
        let kind = Kind::of(ram.u(addr + 20)?);
        let n = nin + nout + nscr;
        let ptrs: Vec<u32> = if n < MAX_WIRES { (0..n as u32).map(|k| ram.u(io + 4 * k)).collect::<Result<_, _>>()? } else { Vec::new() };
        let ins = ptrs.iter().take(nin).copied().collect();
        let outs = ptrs.iter().skip(nin).take(nout).copied().collect();
        let mut children = Vec::new();
        if flags == FLAG_CONTAINER {
            let (count, list) = (ram.u(addr + 52)?, ram.u(addr + 56)?);
            if count as usize > MAX_WIRES * 4 {
                return Err(ResponseError::Graph(format!("container 0x{addr:08x} has {count} children")));
            }
            for k in 0..count {
                let off = ram.u(list + 4 * k)?;
                if off != 0 {
                    children.push(Self::read_at(ram, addr.wrapping_add(4 * off), depth + 1)?);
                }
            }
        }
        Ok(Self { addr, nin, nout, flags, kind, ins, outs, params: addr + 28, children })
    }

    /// Depth first (execution order).
    pub fn find(&self, addr: u32) -> Option<&Module> {
        if self.addr == addr {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(addr))
    }
}

/// One coefficient set of a biquad module: H(z) = g · Π (1 + b1 z⁻¹ + b2 z⁻²) / (1 − a1 z⁻¹ − a2 z⁻²).
#[derive(Debug, Clone, PartialEq)]
pub struct BiquadSet {
    pub g: f64,
    pub sections: Vec<[f64; 4]>,
}

/// Coefficient sets of a biquad module and the set each channel uses (`re/dsp/stock_model.py`).
pub fn biquad_sets(ram: &Ram, m: &Module) -> Result<(Vec<BiquadSet>, Vec<usize>), ResponseError> {
    let p = m.params;
    let (active, maximum, nsets) = (ram.u(p)?, ram.u(p + 4)?, ram.u(p + 8)?.max(1));
    if active > maximum || maximum > 16 || nsets > 16 {
        return Err(ResponseError::Graph(format!("biquad 0x{:08x}: {active} / {maximum} sections, {nsets} sets", m.addr)));
    }
    let (req, coef) = (ram.u(p + 20)?, ram.u(p + 28)?);
    let stride = 4 * (1 + 4 * maximum);
    let mut sets = Vec::new();
    for s in 0..nsets {
        let base = coef + s * stride;
        let sections = (0..active).map(|k| ram.floats(base + 4 + 16 * k, 4).map(|v| [v[0], v[1], v[2], v[3]])).collect::<Result<_, _>>()?;
        sets.push(BiquadSet { g: ram.f(base)?, sections });
    }
    let chans = (0..m.nin as u32).map(|k| ram.u(req + 4 * k).map(|v| v as usize)).collect::<Result<Vec<_>, _>>()?;
    if chans.iter().any(|&c| c >= sets.len()) {
        return Err(ResponseError::Graph(format!("biquad 0x{:08x}: set index out of range", m.addr)));
    }
    Ok((sets, chans))
}

impl BiquadSet {
    pub fn response(&self, w: &[f64]) -> Vec<super::complex::C64> {
        use super::complex::C64;
        w.iter()
            .map(|&w| {
                let z1 = C64::delay(w);
                let z2 = z1 * z1;
                self.sections.iter().fold(C64::new(self.g, 0.0), |h, s| {
                    let num = C64::ONE + z1.scale(s[0]) + z2.scale(s[1]);
                    let den = C64::ONE + z1.scale(-s[2]) + z2.scale(-s[3]);
                    h * num.div(den)
                })
            })
            .collect()
    }
}
